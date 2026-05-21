//! ssiec-sv-publisher
//!
//! IEC 61850-9-2 LE Sampled Value emitter. Three sinks:
//!
//! - `hex`         (default) — pretty hex dump of one frame to stdout
//! - `pcap <path>`           — libpcap file Wireshark can dissect
//! - `udp <addr:port>`       — UDP datagrams of the L2 frame payload
//!
//! Phase 1 extends `pcap` and `udp` to **continuous emission**: the same
//! [`WaveformConfig`](ssiec_sv_publisher::WaveformConfig) drives an
//! incrementing `smp_cnt` so consumers see a real 3-phase waveform rather
//! than the Phase 0 single-sample snapshot.
//!
//! See `docs/decisions/0003-sv-encoder-design.md` for design rationale
//! and issue #2 for the WBS-6.1 work item.

use std::io;
use std::net::UdpSocket;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use ssiec_sv_publisher::{
    encode_frame, write_hex_dump, AsduFields, FrameParams, PcapWriter, SampleData, WaveformConfig,
    DEFAULT_APPID, MAX_FRAME_BYTES,
};

/// Default svID used by the demo CLI; real deployments source this from
/// the SCD.
const DEMO_SV_ID: &str = "SVDC_DEMO_01";

/// Default confRev for the demo CLI.
const DEMO_CONF_REV: u32 = 1;

/// Default smpSynch = 2 (global, as if PTP-locked).
const DEMO_SMP_SYNCH: u8 = 2;

fn print_help(program: &str) {
    println!("ssiec-sv-publisher — IEC 61850-9-2 LE Sampled Value emitter");
    println!();
    println!("USAGE:");
    println!("    {program} [hex]");
    println!("    {program} pcap <path> [options]");
    println!("    {program} udp  <addr:port> [options]");
    println!("    {program} --help");
    println!();
    println!("MODES:");
    println!("    hex     Default. Write a hex+ASCII dump of one nominal frame to stdout.");
    println!("    pcap    Write a libpcap file. Single frame by default; pass --frames N");
    println!("            to capture a continuous stream Wireshark can scroll through.");
    println!("    udp     Send the L2 frame payload (after the Ethernet header) over UDP.");
    println!("            Single shot by default; pass --frames or --duration for a stream.");
    println!();
    println!("OPTIONS (pcap and udp):");
    println!("    --frames N           Emit N frames with incrementing smp_cnt (default: 1).");
    println!("    --duration SECONDS   udp only: emit for SECONDS at --rate (default: 0).");
    println!("    --rate HZ            Sample rate in Hz (default: 4800 = 80 SPC × 60 Hz).");
    println!("    --frequency HZ       Fundamental frequency (default: 60).");
    println!("    --harmonics 3,5,7    Voltage harmonics, 5% amplitude each (default: none).");
    println!("    --vendor PRESET      Emit frames matching one of `abb_relion_670`,");
    println!("                         `siemens_siprotec_5`, `ge_ur_series`, `sel_2240`.");
    println!("                         `--vendor list` prints the table and exits.");
    println!("    --vendor-icd PATH    Load APPID / MAC / svID / smpRate / VLAN from a");
    println!("                         vendor-supplied SCL file (.icd / .cid / .scd).");
    println!("                         Overrides matching --vendor preset fields.");
    println!("    --calibration-csv P  Load a per-channel calibration table from CSV.");
    println!("                         Printed for inspection (the SVDC daemon writes it");
    println!("                         into the OperationalState through a separate path).");
    println!();
    println!("EXAMPLES:");
    println!("    {program}                                              # hex dump of one frame");
    println!("    {program} pcap stream.pcap --frames 4800                # 1 second @ 4800 Hz");
    println!("    {program} pcap abb.pcap --vendor abb_relion_670 --frames 200");
    println!("    {program} pcap sim.pcap --vendor-icd vendor.icd --frames 100");
    println!("    {program} udp 239.0.0.1:102 --vendor sel_2240 --duration 5");
}

/// Options that apply to `pcap` and `udp` continuous modes.
#[derive(Debug, Clone)]
struct StreamOpts {
    /// Number of frames; for `udp` with `--duration`, derived from rate.
    frames: u32,
    /// Sample rate driving timestamps and (for `udp`) inter-frame sleep.
    sample_rate: u32,
    /// Fundamental frequency for the waveform synthesiser.
    frequency: f32,
    /// Voltage harmonics (each 5% amplitude).
    harmonics: Vec<u32>,
    /// `udp` only: cap by duration instead of by frame count.
    duration: Option<Duration>,
    /// Vendor profile preset name (`abb_relion_670`, `siemens_siprotec_5`,
    /// `ge_ur_series`, `sel_2240`). When set, the frame is emitted with
    /// that vendor's APPID, multicast MAC, svID template, and VLAN tag.
    vendor: Option<&'static ssiec_sv_publisher::VendorProfile>,
    /// Path to a vendor-supplied ICD/SCD XML. Loaded after `vendor` so
    /// the XML fields override the preset.
    vendor_icd: Option<String>,
    /// Path to a vendor calibration CSV. Printed for inspection; the
    /// SVDC's `OperationalState` consumes it via a separate code path.
    calibration_csv: Option<String>,
}

impl Default for StreamOpts {
    fn default() -> Self {
        Self {
            frames: 1,
            sample_rate: 4800,
            frequency: 60.0,
            harmonics: Vec::new(),
            duration: None,
            vendor: None,
            vendor_icd: None,
            calibration_csv: None,
        }
    }
}

impl StreamOpts {
    fn to_waveform(&self) -> WaveformConfig {
        WaveformConfig {
            sample_rate: self.sample_rate,
            frequency: self.frequency,
            voltage_harmonics: self.harmonics.iter().map(|n| (*n, 0.05)).collect(),
            ..WaveformConfig::default()
        }
    }
}

fn parse_stream_opts(rest: &mut impl Iterator<Item = String>) -> Result<StreamOpts, String> {
    let mut opts = StreamOpts::default();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--frames" => {
                let v = rest.next().ok_or("--frames needs a value")?;
                opts.frames = v.parse().map_err(|_| format!("invalid --frames: {v}"))?;
            }
            "--duration" => {
                let v = rest.next().ok_or("--duration needs a value")?;
                let secs: f64 = v.parse().map_err(|_| format!("invalid --duration: {v}"))?;
                opts.duration = Some(Duration::from_secs_f64(secs));
            }
            "--rate" => {
                let v = rest.next().ok_or("--rate needs a value")?;
                opts.sample_rate = v.parse().map_err(|_| format!("invalid --rate: {v}"))?;
            }
            "--frequency" => {
                let v = rest.next().ok_or("--frequency needs a value")?;
                opts.frequency = v.parse().map_err(|_| format!("invalid --frequency: {v}"))?;
            }
            "--harmonics" => {
                let v = rest.next().ok_or("--harmonics needs a value")?;
                for tok in v.split(',') {
                    let n: u32 = tok
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid harmonic order: {tok}"))?;
                    opts.harmonics.push(n);
                }
            }
            "--vendor" => {
                let v = rest.next().ok_or("--vendor needs a value")?;
                if v == "list" {
                    println!("Available vendor presets:");
                    for p in ssiec_sv_publisher::vendor::ALL {
                        println!("  {} — {}", p.name, p.notes);
                    }
                    std::process::exit(0);
                }
                let profile = ssiec_sv_publisher::vendor::lookup(&v)
                    .ok_or_else(|| format!("unknown vendor preset `{v}` (try `--vendor list`)"))?;
                opts.vendor = Some(profile);
                opts.sample_rate = profile.default_smp_rate_hz;
            }
            "--vendor-icd" => {
                let v = rest.next().ok_or("--vendor-icd needs a path")?;
                opts.vendor_icd = Some(v);
            }
            "--calibration-csv" => {
                let v = rest.next().ok_or("--calibration-csv needs a path")?;
                opts.calibration_csv = Some(v);
            }
            other => return Err(format!("unknown option: {other}")),
        }
    }
    if opts.sample_rate == 0 {
        return Err("--rate must be positive".into());
    }
    if opts.frequency <= 0.0 {
        return Err("--frequency must be positive".into());
    }
    Ok(opts)
}

fn build_frame(
    waveform: &WaveformConfig,
    smp_cnt: u32,
    smp_rate_field: u16,
    sv_id: &str,
    conf_rev: u32,
    params: &FrameParams,
    buf: &mut [u8; MAX_FRAME_BYTES],
) -> usize {
    let samples = waveform.sample(smp_cnt);
    let asdu = AsduFields {
        sv_id,
        smp_cnt: smp_cnt as u16, // wraps each 65536 samples; Phase 2 resets per second
        conf_rev,
        smp_synch: DEMO_SMP_SYNCH,
        smp_rate: smp_rate_field,
        samples,
    };
    encode_frame(params, &asdu, buf).expect("encode frame")
}

/// Resolve `--vendor` + `--vendor-icd` into a concrete frame
/// identity. The ICD layers on top of the preset; missing pieces
/// keep the preset's defaults.
fn resolve_frame_identity(opts: &StreamOpts) -> Result<(FrameParams, String, u32, u32), String> {
    if let Some(profile) = opts.vendor {
        let mut effective = *profile;
        if let Some(path) = opts.vendor_icd.as_deref() {
            let loaded = ssiec_sv_publisher::vendor_loader::load_from_icd_path(
                std::path::Path::new(path),
                effective,
            )
            .map_err(|e| format!("--vendor-icd {path}: {e}"))?;
            println!(
                "vendor-icd: loaded fields {:?} from {} (manufacturer = {:?})",
                loaded.overridden, path, loaded.manufacturer
            );
            effective = loaded.profile;
        }
        let sv_id = effective.svid_for("SVDC_DEMO");
        let conf_rev = effective.default_conf_rev;
        let rate = effective.default_smp_rate_hz;
        let params = FrameParams::from_vendor(&effective, [0x00, 0x00, 0x01]);
        Ok((params, sv_id, conf_rev, rate))
    } else if let Some(path) = opts.vendor_icd.as_deref() {
        // ICD without a preset → start from SEL_2240 (strict 9-2 LE
        // baseline) and let the file override fields.
        let loaded = ssiec_sv_publisher::vendor_loader::load_from_icd_path(
            std::path::Path::new(path),
            ssiec_sv_publisher::vendor::SEL_2240,
        )
        .map_err(|e| format!("--vendor-icd {path}: {e}"))?;
        println!(
            "vendor-icd: loaded fields {:?} from {} (manufacturer = {:?})",
            loaded.overridden, path, loaded.manufacturer
        );
        let sv_id = loaded.profile.svid_for("SVDC_DEMO");
        let conf_rev = loaded.profile.default_conf_rev;
        let rate = loaded.profile.default_smp_rate_hz;
        let params = FrameParams::from_vendor(&loaded.profile, [0x00, 0x00, 0x01]);
        Ok((params, sv_id, conf_rev, rate))
    } else {
        Ok((
            FrameParams::DEMO,
            DEMO_SV_ID.to_string(),
            DEMO_CONF_REV,
            opts.sample_rate,
        ))
    }
}

fn maybe_print_calibration(opts: &StreamOpts) {
    if let Some(path) = opts.calibration_csv.as_deref() {
        match ssiec_sv_publisher::calibration_loader::load_csv_path(std::path::Path::new(path)) {
            Ok(rows) => {
                println!(
                    "calibration-csv: loaded {} row(s) from {}",
                    rows.len(),
                    path
                );
                for r in &rows {
                    println!(
                        "  ch{:>2} {:>7} {:>2} ratio={:<14} gain={:.4} offset={:>+7.2} unit_scale={:.5}",
                        r.channel_id, r.quantity, r.phase, r.ct_pt_ratio, r.gain, r.offset, r.unit_scale
                    );
                }
            }
            Err(e) => {
                eprintln!("warning: failed to load --calibration-csv {path}: {e}");
            }
        }
    }
}

fn print_summary(frame: &[u8], opts: Option<&StreamOpts>) {
    println!();
    println!("ssiec-sv-publisher summary:");
    println!("  frame bytes : {}", frame.len());
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
    println!("  svID        : {DEMO_SV_ID}");
    println!("  channels    : Ia Ib Ic In Va Vb Vc Vn");
    if let Some(o) = opts {
        println!("  sample rate : {} Hz", o.sample_rate);
        println!("  fundamental : {:.3} Hz", o.frequency);
        if !o.harmonics.is_empty() {
            println!("  harmonics   : {:?} (5% each)", o.harmonics);
        }
    } else {
        println!("  smpRate     : 4800 (80 SPC * 60 Hz)");
    }
}

fn print_summary_vendor(frame: &[u8], opts: &StreamOpts, sv_id: &str, params: &FrameParams) {
    println!();
    println!("ssiec-sv-publisher summary:");
    println!("  frame bytes : {}", frame.len());
    println!(
        "  dst MAC     : {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        params.dst_mac[0],
        params.dst_mac[1],
        params.dst_mac[2],
        params.dst_mac[3],
        params.dst_mac[4],
        params.dst_mac[5]
    );
    println!(
        "  src MAC     : {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        params.src_mac[0],
        params.src_mac[1],
        params.src_mac[2],
        params.src_mac[3],
        params.src_mac[4],
        params.src_mac[5]
    );
    println!("  APPID       : 0x{:04X}", params.appid);
    if let Some(tag) = params.vlan {
        println!(
            "  VLAN (802.1Q): PCP={}, VID={} (0x{:03X}), DEI={}",
            tag.pcp, tag.vid, tag.vid, tag.dei
        );
    } else {
        println!("  VLAN        : (none)");
    }
    println!("  svID        : {sv_id}");
    println!("  sample rate : {} Hz", opts.sample_rate);
    println!("  fundamental : {:.3} Hz", opts.frequency);
    if !opts.harmonics.is_empty() {
        println!("  harmonics   : {:?} (5% each)", opts.harmonics);
    }
    if let Some(v) = opts.vendor {
        println!("  vendor      : {} — {}", v.name, v.notes);
    }
    println!("  channels    : Ia Ib Ic In Va Vb Vc Vn");
}

fn run_hex() -> io::Result<()> {
    let mut buf = [0u8; MAX_FRAME_BYTES];
    let asdu = AsduFields {
        sv_id: DEMO_SV_ID,
        smp_cnt: 0,
        conf_rev: DEMO_CONF_REV,
        smp_synch: DEMO_SMP_SYNCH,
        smp_rate: 4800,
        samples: SampleData::NOMINAL_3PH,
    };
    let n = encode_frame(&FrameParams::DEMO, &asdu, &mut buf).expect("encode demo frame");
    let frame = &buf[..n];
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    write_hex_dump(&mut lock, frame)?;
    drop(lock);
    print_summary(frame, None);
    Ok(())
}

fn run_pcap(path: &str, opts: &StreamOpts) -> io::Result<()> {
    maybe_print_calibration(opts);
    let (params, sv_id, conf_rev, sample_rate) =
        resolve_frame_identity(opts).map_err(io::Error::other)?;
    let mut opts_eff = opts.clone();
    opts_eff.sample_rate = sample_rate;

    let file = std::fs::File::create(path)?;
    let mut writer = PcapWriter::new(std::io::BufWriter::new(file))?;
    let waveform = opts_eff.to_waveform();
    let smp_rate_field = clamp_smp_rate(opts_eff.sample_rate);
    let mut buf = [0u8; MAX_FRAME_BYTES];
    let mut last_len = 0usize;
    let frame_interval_us = 1_000_000u64 / opts_eff.sample_rate as u64;
    for i in 0..opts_eff.frames {
        let n = build_frame(
            &waveform,
            i,
            smp_rate_field,
            &sv_id,
            conf_rev,
            &params,
            &mut buf,
        );
        last_len = n;
        let ts_us = u64::from(i) * frame_interval_us;
        writer.write_frame(ts_us, &buf[..n])?;
    }
    writer.flush()?;
    let frames = writer.frames_written();
    println!("Wrote {frames} frame(s) to {path}");
    if frames > 1 {
        println!(
            "Open in Wireshark: wireshark {path}  (display filter: sv,  spans {:.3} s)",
            (frames as f64) / (opts_eff.sample_rate as f64)
        );
    } else {
        println!("Open in Wireshark: wireshark {path}");
    }
    if last_len > 0 {
        print_summary_vendor(&buf[..last_len], &opts_eff, &sv_id, &params);
    }
    Ok(())
}

fn run_udp(target: &str, opts: &StreamOpts) -> io::Result<()> {
    maybe_print_calibration(opts);
    let (params, sv_id, conf_rev, sample_rate) =
        resolve_frame_identity(opts).map_err(io::Error::other)?;
    let mut opts_eff = opts.clone();
    opts_eff.sample_rate = sample_rate;
    let waveform = opts_eff.to_waveform();
    let smp_rate_field = clamp_smp_rate(opts_eff.sample_rate);
    let sock = UdpSocket::bind("0.0.0.0:0")?;

    let total_frames: u64 = if let Some(d) = opts_eff.duration {
        let n = (d.as_secs_f64() * opts_eff.sample_rate as f64).round() as u64;
        n.max(1)
    } else {
        u64::from(opts_eff.frames)
    };

    // The UDP payload skips the L2 Ethernet header — including the
    // optional 802.1Q tag — because UDP carries an IP-layer payload.
    let l2_header_len = 14 + if params.vlan.is_some() { 4 } else { 0 };

    let mut buf = [0u8; MAX_FRAME_BYTES];
    let mut last_len = 0usize;
    let frame_interval = Duration::from_nanos(1_000_000_000 / opts_eff.sample_rate as u64);
    let start = Instant::now();
    let mut sent_total = 0u64;
    for i in 0..total_frames {
        let n = build_frame(
            &waveform,
            i as u32,
            smp_rate_field,
            &sv_id,
            conf_rev,
            &params,
            &mut buf,
        );
        last_len = n;
        let sent = sock.send_to(&buf[l2_header_len..n], target)?;
        sent_total += sent as u64;
        if total_frames > 1 {
            let target_time = frame_interval * (i + 1) as u32;
            let elapsed = start.elapsed();
            if target_time > elapsed {
                std::thread::sleep(target_time - elapsed);
            }
        }
    }
    println!(
        "Sent {} frames ({} payload bytes) over UDP to {target} in {:.3} s",
        total_frames,
        sent_total,
        start.elapsed().as_secs_f64()
    );
    println!("(L2 Ethernet header omitted; raw AF_PACKET emission lands in Phase 5.)");
    if last_len > 0 {
        print_summary_vendor(&buf[..last_len], &opts_eff, &sv_id, &params);
    }
    Ok(())
}

/// `smpRate` in 9-2 LE is a 16-bit field. Above 65535 Hz the spec
/// requires a different encoding; the demo CLI just clamps and warns.
fn clamp_smp_rate(rate: u32) -> u16 {
    if rate > u16::MAX as u32 {
        eprintln!(
            "warning: --rate {rate} exceeds u16; smpRate field clamped to {}",
            u16::MAX
        );
        u16::MAX
    } else {
        rate as u16
    }
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

    let result = match mode.as_deref() {
        None | Some("hex") => run_hex(),
        Some("pcap") => {
            let Some(path) = args.next() else {
                eprintln!("error: pcap mode needs a file path");
                eprintln!();
                print_help(&program);
                return ExitCode::FAILURE;
            };
            match parse_stream_opts(&mut args) {
                Ok(opts) => run_pcap(&path, &opts),
                Err(e) => {
                    eprintln!("error: {e}");
                    eprintln!();
                    print_help(&program);
                    return ExitCode::FAILURE;
                }
            }
        }
        Some("udp") => {
            let Some(target) = args.next() else {
                eprintln!("error: udp mode needs an addr:port");
                eprintln!();
                print_help(&program);
                return ExitCode::FAILURE;
            };
            match parse_stream_opts(&mut args) {
                Ok(opts) => run_udp(&target, &opts),
                Err(e) => {
                    eprintln!("error: {e}");
                    eprintln!();
                    print_help(&program);
                    return ExitCode::FAILURE;
                }
            }
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
