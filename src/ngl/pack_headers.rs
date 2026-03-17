//! N105 document and score header packing (serialization) for NGL files.
//!
//! This module provides serialization functions to convert DocumentHeader and ScoreHeader
//! structs back to their N105 binary format for writing NGL files.
//!
//! Each pack function:
//! - Takes a typed struct reference
//! - Returns Vec<u8> with binary N105 format
//! - Uses big-endian byte ordering (PowerPC)
//! - Respects N105 bitfield layouts (MSB-first on PowerPC)
//!
//! Source: NDocAndCnfgTypes.h, NDocAndCnfgTypesN105.h, Ngale5ProgQuickRef-TN1.txt

use crate::doc_types::{DocumentHeader, ScoreHeader, TEXTSTYLE_SIZE_N105};

/// Pack N105 DOCUMENTHEADER to raw bytes (72 bytes total).
///
/// On-disk layout (big-endian):
/// ```text
/// Offset  Size  Field
/// 0       4     origin (Point: 2xi16)
/// 4       8     paper_rect (Rect: 4xi16)
/// 12      8     orig_paper_rect (Rect: 4xi16)
/// 20      4     hold_origin (Point: 2xi16)
/// 24      8     margin_rect (Rect: 4xi16)
/// 32      4     sheet_origin (Point: 2xi16)
/// 36      2     current_sheet (i16)
/// 38      2     num_sheets (i16)
/// 40      2     first_sheet (i16)
/// 42      2     first_page_number (i16)
/// 44      2     start_page_number (i16)
/// 46      2     num_rows (i16)
/// 48      2     num_cols (i16)
/// 50      2     page_type (i16)
/// 52      2     meas_system (i16)
/// 54      8     header_footer_margins (Rect: 4xi16)
/// 62      8     current_paper (Rect: 4xi16)
/// 70      1     landscape (i8)
/// 71      1     little_endian (u8)
/// ```
///
/// Source: NDocAndCnfgTypes.h lines 86-111, Ngale5ProgQuickRef-TN1.txt:97
pub fn pack_document_header_n105(header: &DocumentHeader) -> Vec<u8> {
    let mut buf = vec![0u8; 72];

    // Helper: write Point (2xi16: v, h)
    fn write_point(buf: &mut [u8], offset: usize, v: i16, h: i16) {
        buf[offset..offset + 2].copy_from_slice(&v.to_be_bytes());
        buf[offset + 2..offset + 4].copy_from_slice(&h.to_be_bytes());
    }

    // Helper: write Rect (4xi16: top, left, bottom, right)
    fn write_rect(buf: &mut [u8], offset: usize, top: i16, left: i16, bottom: i16, right: i16) {
        buf[offset..offset + 2].copy_from_slice(&top.to_be_bytes());
        buf[offset + 2..offset + 4].copy_from_slice(&left.to_be_bytes());
        buf[offset + 4..offset + 6].copy_from_slice(&bottom.to_be_bytes());
        buf[offset + 6..offset + 8].copy_from_slice(&right.to_be_bytes());
    }

    // Offset 0-3: origin
    write_point(&mut buf, 0, header.origin.v, header.origin.h);

    // Offset 4-11: paper_rect
    write_rect(
        &mut buf,
        4,
        header.paper_rect.top,
        header.paper_rect.left,
        header.paper_rect.bottom,
        header.paper_rect.right,
    );

    // Offset 12-19: orig_paper_rect
    write_rect(
        &mut buf,
        12,
        header.orig_paper_rect.top,
        header.orig_paper_rect.left,
        header.orig_paper_rect.bottom,
        header.orig_paper_rect.right,
    );

    // Offset 20-23: hold_origin
    write_point(&mut buf, 20, header.hold_origin.v, header.hold_origin.h);

    // Offset 24-31: margin_rect
    write_rect(
        &mut buf,
        24,
        header.margin_rect.top,
        header.margin_rect.left,
        header.margin_rect.bottom,
        header.margin_rect.right,
    );

    // Offset 32-35: sheet_origin
    write_point(&mut buf, 32, header.sheet_origin.v, header.sheet_origin.h);

    // Offset 36-69: i16 fields
    buf[36..38].copy_from_slice(&header.current_sheet.to_be_bytes());
    buf[38..40].copy_from_slice(&header.num_sheets.to_be_bytes());
    buf[40..42].copy_from_slice(&header.first_sheet.to_be_bytes());
    buf[42..44].copy_from_slice(&header.first_page_number.to_be_bytes());
    buf[44..46].copy_from_slice(&header.start_page_number.to_be_bytes());
    buf[46..48].copy_from_slice(&header.num_rows.to_be_bytes());
    buf[48..50].copy_from_slice(&header.num_cols.to_be_bytes());
    buf[50..52].copy_from_slice(&header.page_type.to_be_bytes());
    buf[52..54].copy_from_slice(&header.meas_system.to_be_bytes());

    // Offset 54-61: header_footer_margins (Rect)
    write_rect(
        &mut buf,
        54,
        header.header_footer_margins.top,
        header.header_footer_margins.left,
        header.header_footer_margins.bottom,
        header.header_footer_margins.right,
    );

    // Offset 62-69: current_paper (Rect)
    write_rect(
        &mut buf,
        62,
        header.current_paper.top,
        header.current_paper.left,
        header.current_paper.bottom,
        header.current_paper.right,
    );

    // Offset 70: landscape (i8, already 0)
    buf[70] = header.landscape as u8;

    // Offset 71: little_endian
    buf[71] = header.little_endian;

    buf
}

/// Pack N105 SCOREHEADER_N105 to raw bytes (2148 bytes total).
///
/// This is a complex structure with multiple sections:
/// - Links and metadata (12 bytes)
/// - Comment (256 bytes)
/// - Configuration flags (18 bytes)
/// - Page number config (10 bytes)
/// - Measure number config (6 bytes)
/// - Font records header (2 bytes)
/// - 15 TextStyle records (540 bytes in N105)
/// - Font table (714 bytes)
/// - Display state (32 bytes)
/// - Spacing map (36 bytes)
/// - System indentation (4 bytes)
/// - Voice table (613 bytes)
///
/// Total: 2148 bytes
///
/// Source: NDocAndCnfgTypesN105.h lines 44-256
pub fn pack_score_header_n105(header: &ScoreHeader) -> Vec<u8> {
    let mut buf = vec![0u8; 2148];
    let mut offset = 0;

    // Section 1: Links and metadata (12 bytes)
    buf[offset..offset + 2].copy_from_slice(&header.head_l.to_be_bytes());
    offset += 2;
    buf[offset..offset + 2].copy_from_slice(&header.tail_l.to_be_bytes());
    offset += 2;
    buf[offset..offset + 2].copy_from_slice(&header.sel_start_l.to_be_bytes());
    offset += 2;
    buf[offset..offset + 2].copy_from_slice(&header.sel_end_l.to_be_bytes());
    offset += 2;
    buf[offset..offset + 2].copy_from_slice(&header.nstaves.to_be_bytes());
    offset += 2;
    buf[offset..offset + 2].copy_from_slice(&header.nsystems.to_be_bytes());
    offset += 2;

    // Section 2: Comment (256 bytes)
    buf[offset..offset + 256].copy_from_slice(&header.comment);
    offset += 256;

    // Section 3: Configuration flags (18 bytes)
    // Offset 268: bitfield byte
    let mut b268: u8 = 0;
    if header.note_ins_feedback != 0 {
        b268 |= 0x80; // feedback:1 (bit 7)
    }
    if header.dont_send_patches != 0 {
        b268 |= 0x40; // dontSendPatches:1 (bit 6)
    }
    if header.saved != 0 {
        b268 |= 0x20; // saved:1 (bit 5)
    }
    if header.named != 0 {
        b268 |= 0x10; // named:1 (bit 4)
    }
    if header.used != 0 {
        b268 |= 0x08; // used:1 (bit 3)
    }
    if header.transposed != 0 {
        b268 |= 0x04; // transposed:1 (bit 2)
    }
    if header.filler_sc1 != 0 {
        b268 |= 0x02; // lyricText:1 (bit 1)
    }
    if header.poly_timbral != 0 {
        b268 |= 0x01; // polyTimbral:1 (bit 0)
    }
    buf[offset] = b268;
    offset += 1;

    // Offset 269: filler_sc2 (was currentPage)
    buf[offset] = header.filler_sc2;
    offset += 1;

    // Offsets 270-287: i16 and i32 fields
    buf[offset..offset + 2].copy_from_slice(&header.space_percent.to_be_bytes());
    offset += 2;
    buf[offset..offset + 2].copy_from_slice(&header.srastral.to_be_bytes());
    offset += 2;
    buf[offset..offset + 2].copy_from_slice(&header.altsrastral.to_be_bytes());
    offset += 2;
    buf[offset..offset + 2].copy_from_slice(&header.tempo.to_be_bytes());
    offset += 2;
    buf[offset..offset + 2].copy_from_slice(&header.channel.to_be_bytes());
    offset += 2;
    buf[offset..offset + 2].copy_from_slice(&header.vel_offset.to_be_bytes());
    offset += 2;
    buf[offset..offset + 4].copy_from_slice(&header.header_str_offset.to_be_bytes());
    offset += 4;
    buf[offset..offset + 4].copy_from_slice(&header.footer_str_offset.to_be_bytes());
    offset += 4;

    // Section 4: Page number config (10 bytes, offset 290)
    let mut b290: u8 = 0;
    if header.top_pgn != 0 {
        b290 |= 0x80; // topPGN:1 (bit 7)
    }
    // hPosPGN:3 (bits 6-4)
    b290 |= (header.h_pos_pgn & 0x07) << 4;
    if header.alternate_pgn != 0 {
        b290 |= 0x08; // alternatePGN:1 (bit 3)
    }
    if header.use_header_footer != 0 {
        b290 |= 0x04; // useHeaderFooter:1 (bit 2)
    }
    // fillerPGN:2 (bits 1-0) - leave as 0
    buf[offset] = b290;
    offset += 1;

    buf[offset] = header.filler_mb as u8;
    offset += 1;

    buf[offset..offset + 2].copy_from_slice(&header.filler2.to_be_bytes());
    offset += 2;

    buf[offset..offset + 2].copy_from_slice(&header.d_indent_other.to_be_bytes());
    offset += 2;

    // Offsets 296-301: i8 fields
    buf[offset] = header.first_names as u8;
    offset += 1;
    buf[offset] = header.other_names as u8;
    offset += 1;
    buf[offset] = header.last_global_font as u8;
    offset += 1;
    buf[offset] = header.x_mn_offset as u8;
    offset += 1;
    buf[offset] = header.y_mn_offset as u8;
    offset += 1;
    buf[offset] = header.x_sys_mn_offset as u8;
    offset += 1;

    // Section 5: Measure number config (6 bytes, offset 302)
    // Offset 302: bitfield i16 (aboveMN:1, sysFirstMN:1, startMNPrint1:1, firstMNNumber:13)
    let mut mn_bits: u16 = 0;
    if header.above_mn != 0 {
        mn_bits |= 0x8000; // aboveMN:1 (bit 15)
    }
    if header.sys_first_mn != 0 {
        mn_bits |= 0x4000; // sysFirstMN:1 (bit 14)
    }
    if header.start_mn_print1 != 0 {
        mn_bits |= 0x2000; // startMNPrint1:1 (bit 13)
    }
    // firstMNNumber:13 (bits 12-0)
    mn_bits |= (header.first_mn_number as u16) & 0x1FFF;
    buf[offset..offset + 2].copy_from_slice(&mn_bits.to_be_bytes());
    offset += 2;

    buf[offset..offset + 2].copy_from_slice(&header.master_head_l.to_be_bytes());
    offset += 2;
    buf[offset..offset + 2].copy_from_slice(&header.master_tail_l.to_be_bytes());
    offset += 2;

    // Section 6: Font records header (2 bytes, offset 308)
    buf[offset] = header.filler1 as u8;
    offset += 1;
    buf[offset] = header.n_font_records as u8;
    offset += 1;

    // Section 7: 15 TextStyle records (540 bytes, offset 310)
    pack_textstyle_records(&mut buf, offset, header);
    offset += 540;

    // Section 8: Font table (714 bytes, offset 850)
    buf[offset..offset + 2].copy_from_slice(&header.nfonts_used.to_be_bytes());
    offset += 2;

    for font_item in &header.font_table {
        buf[offset] = (font_item.font_id as u16).to_be_bytes()[0];
        buf[offset + 1] = (font_item.font_id as u16).to_be_bytes()[1];
        buf[offset + 2..offset + 34].copy_from_slice(&font_item.font_name);
        offset += 34;
    }

    buf[offset..offset + 32].copy_from_slice(&header.mus_font_name);
    offset += 32;

    // Section 9: Display state (32 bytes, offset 1564)
    buf[offset..offset + 2].copy_from_slice(&header.magnify.to_be_bytes());
    offset += 2;
    buf[offset..offset + 2].copy_from_slice(&header.sel_staff.to_be_bytes());
    offset += 2;
    buf[offset] = header.other_mn_staff as u8;
    offset += 1;
    buf[offset] = header.number_meas as u8;
    offset += 1;
    buf[offset..offset + 2].copy_from_slice(&header.current_system.to_be_bytes());
    offset += 2;
    buf[offset..offset + 2].copy_from_slice(&header.space_table.to_be_bytes());
    offset += 2;
    buf[offset..offset + 2].copy_from_slice(&header.htight.to_be_bytes());
    offset += 2;
    buf[offset..offset + 2].copy_from_slice(&header.filler_int.to_be_bytes());
    offset += 2;
    buf[offset..offset + 2].copy_from_slice(&header.look_voice.to_be_bytes());
    offset += 2;
    buf[offset..offset + 2].copy_from_slice(&header.filler_hp.to_be_bytes());
    offset += 2;
    buf[offset..offset + 2].copy_from_slice(&header.filler_lp.to_be_bytes());
    offset += 2;
    buf[offset..offset + 2].copy_from_slice(&header.ledger_y_sp.to_be_bytes());
    offset += 2;
    buf[offset..offset + 2].copy_from_slice(&header.deflam_time.to_be_bytes());
    offset += 2;

    // Boolean flags (11 bytes, offset 1588)
    buf[offset] = header.auto_respace;
    offset += 1;
    buf[offset] = header.insert_mode;
    offset += 1;
    buf[offset] = header.beam_rests;
    offset += 1;
    buf[offset] = header.graph_mode;
    offset += 1;
    buf[offset] = header.show_syncs;
    offset += 1;
    buf[offset] = header.frame_systems;
    offset += 1;
    buf[offset] = header.filler_em;
    offset += 1;
    buf[offset] = header.color_voices;
    offset += 1;
    buf[offset] = header.show_invis;
    offset += 1;
    buf[offset] = header.show_dur_prob;
    offset += 1;
    buf[offset] = header.record_flats;
    offset += 1;

    // Section 10: Spacing map (36 bytes, offset 1596)
    for space in header.space_map.iter() {
        buf[offset..offset + 4].copy_from_slice(&space.to_be_bytes());
        offset += 4;
    }

    // Section 11: System indentation (4 bytes, offset 1632)
    buf[offset..offset + 2].copy_from_slice(&header.d_indent_first.to_be_bytes());
    offset += 2;
    buf[offset..offset + 2].copy_from_slice(&header.y_between_sys.to_be_bytes());
    offset += 2;

    // Section 12: Voice table (512 bytes, offset 1636)
    // VoiceInfo is 2 bytes each in N105: partn (u8) + bitfield byte
    // 256 voices × 2 bytes = 512 bytes total (fills buffer to exactly 2148)
    for voice_info in header.voice_tab.iter() {
        buf[offset] = voice_info.partn;
        offset += 1;

        // Encode bitfield: voiceRole:3 (bits 7-5) | relVoice:5 (bits 4-0)
        let mut bfield: u8 = 0;
        bfield |= (voice_info.voice_role & 0x07) << 5;
        bfield |= voice_info.rel_voice & 0x1F;
        buf[offset] = bfield;
        offset += 1;
    }

    // Note: expansion array is NOT part of the N105 SCOREHEADER_N105 structure.
    // It is written separately in OG Nightingale as part of extended document data.
    // The voice_tab (256 voices × 2 bytes = 512 bytes) fills the remaining space
    // from offset 1636 to exactly 2148 bytes.

    buf
}

/// Pack 15 TextStyle records to N105 format (540 bytes total, 36 bytes each).
///
/// Each TextStyle record (36 bytes):
/// - fontName[32]: Pascal string (length byte + up to 31 chars)
/// - Bitfield u16: filler:5, lyric:1, enclosure:2, relFSize:1, fontSize:7
/// - fontStyle (i16)
///
/// Source: NDocAndCnfgTypesN105.h lines 153-165
fn pack_textstyle_records(buf: &mut [u8], base_offset: usize, header: &ScoreHeader) {
    let textstyles = [
        // MN
        (
            &header.font_name_mn,
            header.lyric_mn,
            header.enclosure_mn,
            header.rel_f_size_mn,
            header.font_size_mn,
            header.font_style_mn,
        ),
        // PN
        (
            &header.font_name_pn,
            header.lyric_pn,
            header.enclosure_pn,
            header.rel_f_size_pn,
            header.font_size_pn,
            header.font_style_pn,
        ),
        // RM
        (
            &header.font_name_rm,
            header.lyric_rm,
            header.enclosure_rm,
            header.rel_f_size_rm,
            header.font_size_rm,
            header.font_style_rm,
        ),
        // R1
        (
            &header.font_name1,
            header.lyric1,
            header.enclosure1,
            header.rel_f_size1,
            header.font_size1,
            header.font_style1,
        ),
        // R2
        (
            &header.font_name2,
            header.lyric2,
            header.enclosure2,
            header.rel_f_size2,
            header.font_size2,
            header.font_style2,
        ),
        // R3
        (
            &header.font_name3,
            header.lyric3,
            header.enclosure3,
            header.rel_f_size3,
            header.font_size3,
            header.font_style3,
        ),
        // R4
        (
            &header.font_name4,
            header.lyric4,
            header.enclosure4,
            header.rel_f_size4,
            header.font_size4,
            header.font_style4,
        ),
        // TM
        (
            &header.font_name_tm,
            header.lyric_tm,
            header.enclosure_tm,
            header.rel_f_size_tm,
            header.font_size_tm,
            header.font_style_tm,
        ),
        // CS
        (
            &header.font_name_cs,
            header.lyric_cs,
            header.enclosure_cs,
            header.rel_f_size_cs,
            header.font_size_cs,
            header.font_style_cs,
        ),
        // PG
        (
            &header.font_name_pg,
            header.lyric_pg,
            header.enclosure_pg,
            header.rel_f_size_pg,
            header.font_size_pg,
            header.font_style_pg,
        ),
        // R5
        (
            &header.font_name5,
            header.lyric5,
            header.enclosure5,
            header.rel_f_size5,
            header.font_size5,
            header.font_style5,
        ),
        // R6
        (
            &header.font_name6,
            header.lyric6,
            header.enclosure6,
            header.rel_f_size6,
            header.font_size6,
            header.font_style6,
        ),
        // R7
        (
            &header.font_name7,
            header.lyric7,
            header.enclosure7,
            header.rel_f_size7,
            header.font_size7,
            header.font_style7,
        ),
        // R8
        (
            &header.font_name8,
            header.lyric8,
            header.enclosure8,
            header.rel_f_size8,
            header.font_size8,
            header.font_style8,
        ),
        // R9
        (
            &header.font_name9,
            header.lyric9,
            header.enclosure9,
            header.rel_f_size9,
            header.font_size9,
            header.font_style9,
        ),
    ];

    for (i, (font_name, lyric, enclosure, rel_f_size, font_size, font_style)) in
        textstyles.iter().enumerate()
    {
        let offset = base_offset + i * TEXTSTYLE_SIZE_N105;

        // Offset +0-31: fontName (Pascal string, 32 bytes)
        buf[offset..offset + 32].copy_from_slice(&font_name[..]);

        // Offset +32-33: Bitfield u16 (big-endian)
        // Bits 15-11: filler (5 bits)
        // Bit 10: lyric (1 bit)
        // Bits 9-8: enclosure (2 bits)
        // Bit 7: relFSize (1 bit)
        // Bits 6-0: fontSize (7 bits)
        let mut flags: u16 = 0;
        // filler in bits 15-11: leave as 0
        if *lyric != 0 {
            flags |= 0x0400; // bit 10
        }
        // enclosure in bits 9-8
        flags |= (*enclosure & 0x03) << 8;
        if *rel_f_size != 0 {
            flags |= 0x0080; // bit 7
        }
        // fontSize in bits 6-0
        flags |= *font_size & 0x7F;

        buf[offset + 32..offset + 34].copy_from_slice(&flags.to_be_bytes());

        // Offset +34-35: fontStyle (i16)
        buf[offset + 34..offset + 36].copy_from_slice(&font_style.to_be_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basic_types::Point;
    use crate::doc_types::FontItem;

    #[test]
    fn test_pack_document_header_basic() {
        let header = DocumentHeader {
            origin: Point { v: 0, h: 0 },
            paper_rect: Default::default(),
            orig_paper_rect: Default::default(),
            hold_origin: Default::default(),
            margin_rect: Default::default(),
            sheet_origin: Default::default(),
            current_sheet: 0,
            num_sheets: 1,
            first_sheet: 0,
            first_page_number: 1,
            start_page_number: 1,
            num_rows: 1,
            num_cols: 1,
            page_type: 0,
            meas_system: 0,
            header_footer_margins: Default::default(),
            current_paper: Default::default(),
            landscape: 0,
            little_endian: 0,
        };

        let packed = pack_document_header_n105(&header);
        assert_eq!(packed.len(), 72, "DocumentHeader must be 72 bytes");

        // Verify num_sheets field at offset 38
        assert_eq!(i16::from_be_bytes([packed[38], packed[39]]), 1);
    }

    #[test]
    fn test_pack_score_header_basic() {
        // Create a minimal ScoreHeader with defaults
        let header = ScoreHeader {
            head_l: 0,
            tail_l: 0,
            sel_start_l: 0,
            sel_end_l: 0,
            nstaves: 1,
            nsystems: 1,
            comment: [0; 256],
            note_ins_feedback: 0,
            dont_send_patches: 0,
            saved: 1,
            named: 1,
            used: 0,
            transposed: 0,
            filler_sc1: 0,
            poly_timbral: 0,
            filler_sc2: 0,
            space_percent: 100,
            srastral: 2,
            altsrastral: 0,
            tempo: 120,
            channel: 0,
            vel_offset: 0,
            header_str_offset: 0,
            footer_str_offset: 0,
            top_pgn: 0,
            h_pos_pgn: 0,
            alternate_pgn: 0,
            use_header_footer: 0,
            filler_pgn: 0,
            filler_mb: 0,
            filler2: 0,
            d_indent_other: 0,
            first_names: 0,
            other_names: 0,
            last_global_font: 0,
            x_mn_offset: 0,
            y_mn_offset: 0,
            x_sys_mn_offset: 0,
            above_mn: 0,
            sys_first_mn: 0,
            start_mn_print1: 0,
            first_mn_number: 1,
            master_head_l: 0,
            master_tail_l: 0,
            filler1: 0,
            n_font_records: 15,
            font_name_mn: [0; 32],
            filler_mn: 0,
            lyric_mn: 0,
            enclosure_mn: 0,
            rel_f_size_mn: 0,
            font_size_mn: 0,
            font_style_mn: 0,
            font_name_pn: [0; 32],
            filler_pn: 0,
            lyric_pn: 0,
            enclosure_pn: 0,
            rel_f_size_pn: 0,
            font_size_pn: 0,
            font_style_pn: 0,
            font_name_rm: [0; 32],
            filler_rm: 0,
            lyric_rm: 0,
            enclosure_rm: 0,
            rel_f_size_rm: 0,
            font_size_rm: 0,
            font_style_rm: 0,
            font_name1: [0; 32],
            filler_r1: 0,
            lyric1: 0,
            enclosure1: 0,
            rel_f_size1: 0,
            font_size1: 0,
            font_style1: 0,
            font_name2: [0; 32],
            filler_r2: 0,
            lyric2: 0,
            enclosure2: 0,
            rel_f_size2: 0,
            font_size2: 0,
            font_style2: 0,
            font_name3: [0; 32],
            filler_r3: 0,
            lyric3: 0,
            enclosure3: 0,
            rel_f_size3: 0,
            font_size3: 0,
            font_style3: 0,
            font_name4: [0; 32],
            filler_r4: 0,
            lyric4: 0,
            enclosure4: 0,
            rel_f_size4: 0,
            font_size4: 0,
            font_style4: 0,
            font_name_tm: [0; 32],
            filler_tm: 0,
            lyric_tm: 0,
            enclosure_tm: 0,
            rel_f_size_tm: 0,
            font_size_tm: 0,
            font_style_tm: 0,
            font_name_cs: [0; 32],
            filler_cs: 0,
            lyric_cs: 0,
            enclosure_cs: 0,
            rel_f_size_cs: 0,
            font_size_cs: 0,
            font_style_cs: 0,
            font_name_pg: [0; 32],
            filler_pg: 0,
            lyric_pg: 0,
            enclosure_pg: 0,
            rel_f_size_pg: 0,
            font_size_pg: 0,
            font_style_pg: 0,
            font_name5: [0; 32],
            filler_r5: 0,
            lyric5: 0,
            enclosure5: 0,
            rel_f_size5: 0,
            font_size5: 0,
            font_style5: 0,
            font_name6: [0; 32],
            filler_r6: 0,
            lyric6: 0,
            enclosure6: 0,
            rel_f_size6: 0,
            font_size6: 0,
            font_style6: 0,
            font_name7: [0; 32],
            filler_r7: 0,
            lyric7: 0,
            enclosure7: 0,
            rel_f_size7: 0,
            font_size7: 0,
            font_style7: 0,
            font_name8: [0; 32],
            filler_r8: 0,
            lyric8: 0,
            enclosure8: 0,
            rel_f_size8: 0,
            font_size8: 0,
            font_style8: 0,
            font_name9: [0; 32],
            filler_r9: 0,
            lyric9: 0,
            enclosure9: 0,
            rel_f_size9: 0,
            font_size9: 0,
            font_style9: 0,
            nfonts_used: 0,
            font_table: [FontItem {
                font_id: 0,
                font_name: [0; 32],
            }; 20],
            mus_font_name: [0; 32],
            magnify: 0,
            sel_staff: 0,
            other_mn_staff: 0,
            number_meas: -1,
            current_system: 0,
            space_table: 0,
            htight: 0,
            filler_int: 0,
            look_voice: 0,
            filler_hp: 0,
            filler_lp: 0,
            ledger_y_sp: 0,
            deflam_time: 0,
            auto_respace: 1,
            insert_mode: 1,
            beam_rests: 0,
            graph_mode: 1,
            show_syncs: 0,
            frame_systems: 0,
            filler_em: 0,
            color_voices: 0,
            show_invis: 0,
            show_dur_prob: 0,
            record_flats: 0,
            space_map: [4096; 9],
            d_indent_first: 0,
            y_between_sys: 0,
            voice_tab: [Default::default(); 101],
            expansion: [Default::default(); 155],
        };

        let packed = pack_score_header_n105(&header);
        assert_eq!(packed.len(), 2148, "ScoreHeader must be 2148 bytes");

        // Verify nstaves at offset 8
        assert_eq!(i16::from_be_bytes([packed[8], packed[9]]), 1);

        // Verify nsystems at offset 10
        assert_eq!(i16::from_be_bytes([packed[10], packed[11]]), 1);
    }
}
