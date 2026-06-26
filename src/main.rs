use anyhow::Result;
use clap::Parser;
use hex::decode;
use log::{debug, info, trace};
use std::ffi::c_void;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::windows::io::FromRawHandle;

use windows::Win32::Foundation::*;
use windows::Win32::System::Console::*;
use windows::Win32::System::Threading::*;
use windows::Win32::System::TpmBaseServices::*;

extern "C" {
    fn _open_osfhandle(osfhandle: isize, flags: i32) -> i32;
}

const TPM2_HEADER_SIZE: usize = 10;
const TPM2_RESPONSE_SIZE_OFFSET: usize = 2;
const TPM2_MAX_COMMAND_SIZE: usize = 4096;

unsafe fn force_bind_stdio() {
    let h_stdin = GetStdHandle(STD_INPUT_HANDLE).unwrap();
    let h_stdout = GetStdHandle(STD_OUTPUT_HANDLE).unwrap();

    let mut dup_in: HANDLE = HANDLE::default();
    let mut dup_out: HANDLE = HANDLE::default();

    let _ = DuplicateHandle(
        GetCurrentProcess(),
        h_stdin,
        GetCurrentProcess(),
        &mut dup_in,
        0,
        true,
        DUPLICATE_SAME_ACCESS,
    );

    let _ = DuplicateHandle(
        GetCurrentProcess(),
        h_stdout,
        GetCurrentProcess(),
        &mut dup_out,
        0,
        true,
        DUPLICATE_SAME_ACCESS,
    );

    let fd_in = _open_osfhandle(dup_in.0 as isize, 0);
    let fd_out = _open_osfhandle(dup_out.0 as isize, 0);

    let stdin_file = File::from_raw_handle(dup_in.0);
    let stdout_file = File::from_raw_handle(dup_out.0);

    std::mem::forget(stdin_file);
    std::mem::forget(stdout_file);

    libc::dup2(fd_in, 0);
    libc::dup2(fd_out, 1);
}

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    input: Option<String>,

    #[arg(short, long)]
    output: Option<String>,

    #[arg(long)]
    hex: bool,

    #[arg(long)]
    bin: bool,

    #[arg(short = 'v', action = clap::ArgAction::Count)]
    verbose: u8,
}

fn init_logging(level: u8) {
    let log_level = match level {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    std::env::set_var("RUST_LOG", log_level);
    env_logger::init();
}

fn open_input(path: &Option<String>) -> Result<Box<dyn Read>> {
    Ok(match path {
        None => Box::new(io::stdin()),
        Some(p) => Box::new(File::open(p)?),
    })
}

fn open_output(path: &Option<String>) -> Result<Box<dyn Write>> {
    Ok(match path {
        None => Box::new(io::stdout()),
        Some(p) => Box::new(File::create(p)?),
    })
}

fn is_hex(s: &[u8]) -> bool {
    s.iter().all(|b| b.is_ascii_hexdigit())
}

fn read_exact_block<R: Read>(r: &mut R, buf: &mut [u8]) -> Result<()> {
    let mut read = 0;
    while read < buf.len() {
        let n = r.read(&mut buf[read..])?;
        if n == 0 {
            if read == 0 {
                return Err(anyhow::anyhow!("EOF"));
            }
            return Err(anyhow::anyhow!("Unexpected EOF"));
        }
        read += n;
    }
    Ok(())
}

fn read_tpm_command<R: Read>(r: &mut R) -> Result<Option<Vec<u8>>> {
    let mut header = [0u8; TPM2_HEADER_SIZE];

    match read_exact_block(r, &mut header) {
        Ok(_) => {}
        Err(_) => return Ok(None),
    }

    let hex_mode = is_hex(&header);
    trace!("header raw: {:?}", String::from_utf8_lossy(&header));

    let mut cmd = if hex_mode {
        let mut hex_header = [0u8; TPM2_HEADER_SIZE * 2];
        hex_header[..TPM2_HEADER_SIZE].copy_from_slice(&header);
        read_exact_block(r, &mut hex_header[TPM2_HEADER_SIZE..])?;
        decode(hex_header)?
    } else {
        header.to_vec()
    };

    let size = u32::from_be_bytes(
        cmd[TPM2_RESPONSE_SIZE_OFFSET..TPM2_RESPONSE_SIZE_OFFSET + 4]
            .try_into()
            .unwrap(),
    ) as usize;

    trace!("parsed command size: {}", size);

    let body_len = size - TPM2_HEADER_SIZE;

    if body_len > 0 {
        if hex_mode {
            let mut body_hex = vec![0u8; body_len * 2];
            read_exact_block(r, &mut body_hex)?;
            cmd.extend_from_slice(&decode(&body_hex)?);
        } else {
            let mut body = vec![0u8; body_len];
            read_exact_block(r, &mut body)?;
            cmd.extend_from_slice(&body);
        }
    }

    trace!("full command [{}]: {:02x?}", cmd.len(), cmd);

    Ok(Some(cmd))
}

fn tbs_init() -> Result<*mut c_void> {
    let mut ctx: *mut c_void = std::ptr::null_mut();

    let params = TBS_CONTEXT_PARAMS2 {
        version: TPM_VERSION_20,
        Anonymous: TBS_CONTEXT_PARAMS2_0 { asUINT32: 1 << 2 },
    };

    trace!("calling Tbsi_Context_Create");

    let res = unsafe {
        Tbsi_Context_Create(
            &params as *const _ as *const TBS_CONTEXT_PARAMS,
            &mut ctx as *mut *mut c_void,
        )
    };

    trace!("Tbsi_Context_Create returned: {:#x}", res);

    if res != 0 {
        return Err(anyhow::anyhow!(format!(
            "Tbsi_Context_Create failed: {res:04x}"
        )));
    }

    Ok(ctx)
}

fn tbs_transceive(ctx: *mut c_void, tx: &[u8], rx: &mut [u8]) -> Result<usize> {
    trace!("calling Tbsip_Submit_Command with {} bytes", tx.len());
    trace!("tx: {:02x?}", tx);

    let mut out_len = rx.len() as u32;

    let res = unsafe {
        Tbsip_Submit_Command(
            ctx as *const c_void,
            TBS_COMMAND_LOCALITY_ZERO,
            TBS_COMMAND_PRIORITY_NORMAL,
            tx,
            rx.as_mut_ptr(),
            &mut out_len,
        )
    };

    trace!("Tbsip_Submit_Command returned: {:#x}", res);
    trace!("out_len: {}", out_len);

    if res != 0 {
        return Err(anyhow::anyhow!(format!(
            "Tbsip_Submit_Command failed: {res:04x}"
        )));
    }

    Ok(out_len as usize)
}

fn main() -> Result<()> {
    unsafe {
        force_bind_stdio();
    }

    let args = Args::parse();
    init_logging(args.verbose);

    info!("starting tpm2-send-tbs");

    let mut input = open_input(&args.input)?;
    let mut output = open_output(&args.output)?;

    let ctx = tbs_init()?;
    let mut rx_buf = vec![0u8; TPM2_MAX_COMMAND_SIZE * 2];

    let output_hex = args.hex;

    while let Some(cmd) = read_tpm_command(&mut input)? {
        debug!("cmd [{}]: {:02x?}", cmd.len(), cmd);

        let rx_len = tbs_transceive(ctx, &cmd, &mut rx_buf)?;
        let rsp = &rx_buf[..rx_len];

        debug!("rsp [{}]: {:02x?}", rsp.len(), rsp);

        if output_hex {
            write!(output, "{}", hex::encode(rsp))?;
        } else {
            output.write_all(rsp)?;
        }
        output.flush()?;
    }

    unsafe {
        Tbsip_Context_Close(ctx as *const c_void);
    }

    Ok(())
}
