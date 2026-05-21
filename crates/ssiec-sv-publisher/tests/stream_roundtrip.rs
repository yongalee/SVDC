//! End-to-end integration: drive `WaveformConfig` → `encode_frame` →
//! `PcapWriter` → `decode_frame`, asserting the resulting capture round-
//! trips with monotonically incrementing `smp_cnt` and waveform-derived
//! samples that match what the synthesiser produced.

use ssiec_sv_publisher::{
    decode_frame, encode_frame, AsduFields, FrameParams, PcapWriter, WaveformConfig,
    MAX_FRAME_BYTES,
};

#[test]
fn pcap_stream_decodes_with_monotonic_smp_cnt_and_matching_samples() {
    let waveform = WaveformConfig::default();
    let total: u32 = 240; // 3 cycles at 80 SPC

    let mut writer = PcapWriter::new(Vec::<u8>::new()).unwrap();
    for i in 0..total {
        let samples = waveform.sample(i);
        let asdu = AsduFields {
            sv_id: "ROUNDTRIP",
            smp_cnt: i as u16,
            conf_rev: 1,
            smp_synch: 2,
            smp_rate: 4800,
            samples,
        };
        let mut frame = [0u8; MAX_FRAME_BYTES];
        let n = encode_frame(&FrameParams::DEMO, &asdu, &mut frame).unwrap();
        let ts_us = (i as u64) * 1_000_000 / (waveform.sample_rate as u64);
        writer.write_frame(ts_us, &frame[..n]).unwrap();
    }
    writer.flush().unwrap();
    assert_eq!(writer.frames_written(), total as u64);
    let buffer: Vec<u8> = writer.into_inner();

    // Walk the pcap and decode each record.
    assert_eq!(&buffer[..4], &[0xD4, 0xC3, 0xB2, 0xA1]);
    let mut pos = 24usize;
    let mut decoded_cnt = 0u32;
    let mut last_smp_cnt: Option<u16> = None;
    while pos < buffer.len() {
        let incl = u32::from_le_bytes(buffer[pos + 8..pos + 12].try_into().unwrap()) as usize;
        let frame = &buffer[pos + 16..pos + 16 + incl];
        let decoded = decode_frame(frame).unwrap();

        // smp_cnt is strictly i (no wrap inside 240).
        assert_eq!(
            u32::from(decoded.asdu.smp_cnt),
            decoded_cnt,
            "smp_cnt should be {decoded_cnt} at record {decoded_cnt}, got {}",
            decoded.asdu.smp_cnt
        );
        if let Some(prev) = last_smp_cnt {
            assert_eq!(
                decoded.asdu.smp_cnt,
                prev.wrapping_add(1),
                "smp_cnt must increment by 1"
            );
        }
        last_smp_cnt = Some(decoded.asdu.smp_cnt);

        // Samples must equal what the synthesiser would produce.
        let expected = waveform.sample(decoded_cnt);
        assert_eq!(
            decoded.asdu.samples, expected,
            "sample mismatch at smp_cnt={decoded_cnt}"
        );

        pos += 16 + incl;
        decoded_cnt += 1;
    }
    assert_eq!(decoded_cnt, total);
}
