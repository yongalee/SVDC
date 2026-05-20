//! ssiec-sv-publisher
//!
//! Phase 0 entry point: emit one valid IEC 61850-9-2 LE Sampled Value
//! frame to one of three sinks:
//!
//! - `hex`         (default) — pretty hex dump to stdout
//! - `pcap <path>`           — write a libpcap file Wireshark can dissect
//! - `udp <addr:port>`       — UDP unicast of the L2 frame payload
//!
//! See `docs/decisions/0003-sv-encoder-design.md` for design rationale
//! and issue #2 for the WBS-6.1 work item.

use std::io::{self, Write};
use std::net::UdpSocket;
use std::process::ExitCode;

use ssiec_sv_publisher::{
    encode_frame, write_hex_dump, write_pcap, AsduFields, FrameParams, SampleData, DEFAULT_APPID,
    MAX_FRAME_BYTES,
};

fn print_help(program: &str) {
    println!("ssiec-sv-publisher — Phase 0 IEC 61850-9-2 LE single-packet emitter");
    println!();
    println!("USAGE:");
    println!("    {program} [hex]");
    println!("    {program} pcap <path>");
    println!("    {program} udp  <addr:port>");
    println!("    {program} --help");
    println!();
    println!("MODES:");
    println!("    hex     Default. Write a hex+ASCII dump of the encoded frame to stdout.");
    println!("    pcap    Write a single-record libpcap file. Open in Wireshark to see");
    println!("            the frame dissected as `IEC 61850-9-2 Sampled Values`.");
    println!("    udp     Send the L2 frame payload (after the Ethernet header) over UDP");
    println!("            to the given address. Useful on Windows where raw L2 emission");
    println!("            needs Npcap and admin privileges.");
    println!();
    println!("EXAMPLES:");
    println!("    {program}                       # hex dump to stdout");
    println!("    {program} pcap sv-demo.pcap     # open in Wireshark");
    println!("    {program} udp  239.0.0.1:102    # multicast unicast demo");
}

fn build_demo_frame(buf: &mut [u8; MAX_FRAME_BYTES]) -> usize {
    let asdu = AsduFields {
        sv_id: "SVDC_DEMO_01",
        smp_cnt: 0,
        conf_rev: 1,
        smp_synch: 2,
        smp_rate: 4800,
        samples: SampleData::NOMINAL_3PH,
    };
    encode_frame(&FrameParams::DEMO, &asdu, buf).expect("encode demo frame")
}

fn print_summary(frame: &[u8]) {
    println!();
    println!("Phase 0 demo frame (WBS-6.1):");
    println!("  total bytes : {}", frame.len());
    println!(
        "  dst MAC     : {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        frame[0], frame[1], frame[2], frame[3], frame[4], frame[5]
    );
    println!(
        "  src MAC     : {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        frame[6], frame[7], frame[8], frame[9], frame[10], frame[11]
    );
    println!("  ethertype   : 0x{:02X}{:02X}", frame[12], frame[13]);
    println!("  APPID       : 0x{:04X}", DEFAULT_APPID);
    println!("  svID        : SVDC_DEMO_01");
    println!("  smpRate     : 4800 (80 SPC * 60 Hz)");
    println!("  channels    : Ia Ib Ic In Va Vb Vc Vn (nominal, balanced 3-phase)");
}

fn run_hex(frame: &[u8]) -> io::Result<()> {
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    write_hex_dump(&mut lock, frame)?;
    drop(lock);
    print_summary(frame);
    Ok(())
}

fn run_pcap(frame: &[u8], path: &str) -> io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    write_pcap(&mut file, frame)?;
    file.flush()?;
    println!("Wrote {} bytes of pcap to {path}", 24 + 16 + frame.len());
    println!("Open in Wireshark:");
    println!("    wireshark {path}");
    println!("The frame dissects as `IEC 61850-9-2 Sampled Values`.");
    print_summary(frame);
    Ok(())
}

fn run_udp(frame: &[u8], target: &str) -> io::Result<()> {
    let payload = &frame[14..];
    let sock = UdpSocket::bind("0.0.0.0:0")?;
    let sent = sock.send_to(payload, target)?;
    println!("Sent {sent} bytes of SV payload via UDP to {target}");
    println!("(L2 Ethernet header omitted; Phase 1 will add raw AF_PACKET on Linux.)");
    print_summary(frame);
    Ok(())
}

fn main() -> ExitCode {
    let mut args = std::env::args();
    let program = args
        .next()
        .unwrap_or_else(|| "ssiec-sv-publisher".to_string());
    let mode = args.next();

    if matches!(mode.as_deref(), Some("--help") | Some("-h") | Some("help")) {
        print_help(&program);
        return ExitCode::SUCCESS;
    }

    let mut buf = [0u8; MAX_FRAME_BYTES];
    let n = build_demo_frame(&mut buf);
    let frame = &buf[..n];

    let result = match mode.as_deref() {
        None | Some("hex") => run_hex(frame),
        Some("pcap") => {
            let Some(path) = args.next() else {
                eprintln!("error: pcap mode needs a file path");
                eprintln!();
                print_help(&program);
                return ExitCode::FAILURE;
            };
            run_pcap(frame, &path)
        }
        Some("udp") => {
            let Some(target) = args.next() else {
                eprintln!("error: udp mode needs an addr:port");
                eprintln!();
                print_help(&program);
                return ExitCode::FAILURE;
            };
            run_udp(frame, &target)
        }
        Some(other) => {
            eprintln!("error: unknown mode `{other}`");
            eprintln!();
            print_help(&program);
            return ExitCode::FAILURE;
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
