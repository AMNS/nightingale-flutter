//! Shared layout configuration and post-hoc score layout engine.
//!
//! Provides `LayoutConfig` (page geometry, staff sizing, margins) and
//! `layout_score()` which assigns horizontal positions (xd) to all
//! Measures and Syncs in an InterpretedScore using the Gourlay spacing
//! algorithm, creates multi-system breaks, and fixes up geometry.
//!
//! Used by both the Notelist and MusicXML import pipelines to produce
//! renderable scores.

use crate::basic_types::{DRect, Ddist, KsInfo, Link, NILINK};
use crate::defs::RESFACTOR;
use crate::duration::code_to_l_dur;
use crate::ngl::interpret::{InterpretedObject, InterpretedScore, ObjData};
use crate::obj_types::{
    AClef, AConnect, AKeySig, AStaff, Clef, Connect, KeySig, ObjectHeader, Page, Staff,
    SubObjHeader, System, SHOW_ALL_LINES,
};
use crate::space_time::{
    min_measure_width_stdist, respace_1bar, stdist_to_ddist, sync_width_left, sync_width_right,
    NoteWidthInfo, SpaceTimeInfo, CONFIG_SP_AFTER_BAR, J_IT,
};
use crate::utility::DFLT_XMOVEACC;

// ============================================================================
// LayoutConfig
// ============================================================================

/// Shared layout configuration for score rendering.
///
/// Matches Nightingale defaults (US Letter, rastral 5, 1" margins).
/// Used by both Notelist (`to_score.rs`) and MusicXML import.
#[derive(Debug, Clone)]
pub struct LayoutConfig {
    /// Page width in points (US Letter = 612).
    pub page_width: i16,
    /// Page height in points (US Letter = 792).
    pub page_height: i16,
    /// Staff height in DDIST (rastral 5 = 384 DDIST = 24pt).
    pub staff_height: Ddist,
    /// System left margin in DDIST.
    pub system_left: Ddist,
    /// System right limit in DDIST.
    pub system_right: Ddist,
    /// System top margin in DDIST.
    pub system_top: Ddist,
    /// System-to-system vertical spacing in DDIST.
    pub inter_system: Ddist,
    /// Staff-to-staff spacing (top-to-top) in DDIST.
    pub inter_staff: Ddist,
    /// Maximum measures per system (0 = auto-fit).
    pub max_measures: usize,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        let staff_height: Ddist = 384; // 24pt = rastral 5
        let page_width: i16 = 612;
        let margin_left_pt: i16 = 72; // 1"
        let margin_right_pt: i16 = 54;
        let margin_top_pt: i16 = 72;
        Self {
            page_width,
            page_height: 792,
            staff_height,
            system_left: margin_left_pt * 16,
            system_right: (page_width - margin_right_pt) * 16,
            system_top: margin_top_pt * 16,
            inter_system: 2800,
            inter_staff: staff_height * 5 / 2, // 2.5x staff height
            max_measures: 4,
        }
    }
}

impl LayoutConfig {
    /// Usable music width in DDIST (system_right - system_left).
    pub fn content_width(&self) -> Ddist {
        self.system_right - self.system_left
    }

    /// Inter-line distance in DDIST (staff_height / 4 for 5-line staff).
    pub fn d_line_sp(&self) -> Ddist {
        self.staff_height / 4
    }

    /// How many systems fit on one page.
    pub fn systems_per_page(&self, num_staves: usize) -> usize {
        let system_height = self.staff_height + (num_staves as Ddist - 1) * self.inter_staff;
        let usable_height = (self.page_height * 16) - self.system_top;
        let count = usable_height / self.inter_system.max(system_height + 200);
        (count as usize).max(1)
    }

    /// Standard staff metrics for AStaff records.
    /// Returns (ledger_width, note_head_width, frac_beam_width, flag_leading).
    pub fn staff_metrics(&self) -> (Ddist, Ddist, Ddist, Ddist) {
        let ls = self.d_line_sp();
        (ls, ls, ls / 2, 0)
    }
}

// ============================================================================
// Internal data structures
// ============================================================================

/// Metadata extracted from the first system's preamble.
struct PreambleInfo {
    num_staves: usize,
    clef_types: Vec<i8>,
    ks_info: KsInfo,
    time_num: i8,
    time_denom: i8,
    has_connect: bool,
    connect_type: u8,
    staff_above: i8,
    staff_below: i8,
    system_link: Link,
    page_link: Link,
    tail_link: Link,
}

/// Information about one measure in the score's object list.
struct MeasureInfo {
    obj_link: Link,
    /// Last object in this measure's chain (before the next Measure or Tail).
    last_link: Link,
    sync_links: Vec<Link>,
    sync_times: Vec<i32>,
    sync_note_infos: Vec<Vec<(i8, u8, bool, u8)>>,
}

// ============================================================================
// Main entry point
// ============================================================================

/// Walk the score's object list and assign horizontal positions (xd) to all
/// Measures and Syncs using the Gourlay spacing algorithm. Creates multi-system
/// breaks by splicing new System/Staff/Connect/Clef/KeySig objects into the
/// object list. Also fixes up system geometry and staff dimensions.
pub fn layout_score(score: &mut InterpretedScore, config: &LayoutConfig) {
    // --- 1. Extract preamble metadata from first system ---
    let preamble = match extract_preamble(score) {
        Some(p) => p,
        None => return,
    };

    // --- 2. Collect measures with chain info ---
    let measures = collect_measures(score);
    if measures.is_empty() {
        return;
    }

    // --- 3. Compute spacing (Gourlay per measure) ---
    let (positions_stdist, totals_stdist) = compute_spacing(&measures, config);

    // --- 4. Group measures into systems ---
    let preamble_width = compute_preamble_width_info(&preamble, config);
    let cont_preamble = compute_continuation_preamble_info(&preamble, config);
    let available = config.content_width() - preamble_width;
    let cont_available = config.content_width() - cont_preamble;
    let system_ranges =
        group_measures_into_systems(&totals_stdist, config, available, cont_available);
    if system_ranges.is_empty() {
        return;
    }

    // --- 5. Fix first system's geometry ---
    fix_staff_geometry(score, config);
    fix_system_geometry_at(score, config, &preamble, 0);
    fix_preamble_positions(score, config);

    // --- 6. Insert system breaks for systems 2+ ---
    let sys_per_page = config.systems_per_page(preamble.num_staves);
    insert_system_breaks(
        score,
        config,
        &preamble,
        &measures,
        &system_ranges,
        sys_per_page,
    );

    // --- 7. Scale measures and assign xd positions ---
    assign_xd_positions(
        score,
        &measures,
        &system_ranges,
        &positions_stdist,
        &totals_stdist,
        config,
        preamble_width,
        cont_preamble,
        available,
        cont_available,
    );

    // --- 8. Set page dimensions ---
    score.page_width_pt = config.page_width as f32;
    score.page_height_pt = config.page_height as f32;
}

// ============================================================================
// Preamble extraction
// ============================================================================

fn extract_preamble(score: &InterpretedScore) -> Option<PreambleInfo> {
    let mut info = PreambleInfo {
        num_staves: 0,
        clef_types: Vec::new(),
        ks_info: KsInfo::default(),
        time_num: 4,
        time_denom: 4,
        has_connect: false,
        connect_type: 3,
        staff_above: 1,
        staff_below: 1,
        system_link: NILINK,
        page_link: NILINK,
        tail_link: NILINK,
    };

    // Find tail link
    for (i, obj) in score.objects.iter().enumerate() {
        if matches!(obj.data, ObjData::Tail(_)) {
            info.tail_link = i as Link;
            break;
        }
    }

    for obj in score.walk() {
        match &obj.data {
            ObjData::Page(_) => info.page_link = obj.index,
            ObjData::System(_) => info.system_link = obj.index,
            ObjData::Staff(_) => {
                if let Some(staffs) = score.staffs.get(&obj.header.first_sub_obj) {
                    info.num_staves = staffs.len();
                    info.clef_types = staffs.iter().map(|s| s.clef_type).collect();
                    if let Some(first) = staffs.first() {
                        info.ks_info = first.ks_info;
                        info.time_num = first.numerator;
                        info.time_denom = first.denominator;
                    }
                }
            }
            ObjData::Connect(_) => {
                info.has_connect = true;
                if let Some(conns) = score.connects.get(&obj.header.first_sub_obj) {
                    if let Some(c) = conns.first() {
                        info.connect_type = c.connect_type;
                        info.staff_above = c.staff_above;
                        info.staff_below = c.staff_below;
                    }
                }
            }
            ObjData::Clef(c) => {
                if !c.in_measure {
                    if let Some(clefs) = score.clefs.get(&obj.header.first_sub_obj) {
                        info.clef_types = clefs.iter().map(|c| c.header.sub_type).collect();
                    }
                }
            }
            ObjData::KeySig(ks) => {
                if !ks.in_measure {
                    if let Some(kss) = score.keysigs.get(&obj.header.first_sub_obj) {
                        if let Some(first) = kss.first() {
                            info.ks_info = first.ks_info;
                        }
                    }
                }
            }
            ObjData::TimeSig(_) => {
                if let Some(tss) = score.timesigs.get(&obj.header.first_sub_obj) {
                    if let Some(first) = tss.first() {
                        info.time_num = first.numerator;
                        info.time_denom = first.denominator;
                    }
                }
            }
            ObjData::Measure(_) => break, // Stop at first measure
            _ => {}
        }
    }

    if info.num_staves > 0 {
        Some(info)
    } else {
        None
    }
}

// ============================================================================
// Measure collection (with chain tracking)
// ============================================================================

fn collect_measures(score: &InterpretedScore) -> Vec<MeasureInfo> {
    let mut measures: Vec<MeasureInfo> = Vec::new();
    let mut current_measure: Option<MeasureInfo> = None;
    let mut last_in_measure: Link = NILINK;

    for obj in score.walk() {
        match &obj.data {
            ObjData::Measure(_) => {
                if let Some(mut m) = current_measure.take() {
                    m.last_link = last_in_measure;
                    measures.push(m);
                }
                last_in_measure = obj.index;
                current_measure = Some(MeasureInfo {
                    obj_link: obj.index,
                    last_link: obj.index,
                    sync_links: Vec::new(),
                    sync_times: Vec::new(),
                    sync_note_infos: Vec::new(),
                });
            }
            ObjData::Tail(_) => {
                if let Some(mut m) = current_measure.take() {
                    m.last_link = last_in_measure;
                    measures.push(m);
                }
            }
            _ => {
                if current_measure.is_some() {
                    last_in_measure = obj.index;
                    if let ObjData::Sync(sync_data) = &obj.data {
                        if let Some(ref mut m) = current_measure {
                            m.sync_links.push(obj.index);
                            m.sync_times.push(sync_data.time_stamp as i32);
                            let mut infos: Vec<(i8, u8, bool, u8)> = Vec::new();
                            if let Some(notes) = score.notes.get(&obj.header.first_sub_obj) {
                                for note in notes {
                                    infos.push((
                                        note.header.sub_type,
                                        note.ndots,
                                        note.rest,
                                        note.accident,
                                    ));
                                }
                            }
                            m.sync_note_infos.push(infos);
                        }
                    }
                }
            }
        }
    }

    // Fix timestamps: if all zero, estimate from note durations
    for m in &mut measures {
        if m.sync_times.iter().all(|&t| t == 0) && !m.sync_times.is_empty() {
            let mut t = 0i32;
            for (ei, infos) in m.sync_note_infos.iter().enumerate() {
                m.sync_times[ei] = t;
                let min_dur = infos
                    .iter()
                    .filter(|&&(dur, _, _, _)| dur > 0)
                    .map(|&(dur, dots, _, _)| code_to_l_dur(dur, dots))
                    .filter(|&d| d > 0)
                    .min()
                    .unwrap_or(480);
                t += min_dur;
            }
        }
    }

    measures
}

// ============================================================================
// Spacing computation (Gourlay algorithm)
// ============================================================================

fn compute_spacing(measures: &[MeasureInfo], _config: &LayoutConfig) -> (Vec<Vec<i16>>, Vec<f32>) {
    let space_prop = RESFACTOR * 100;
    let mut positions: Vec<Vec<i16>> = Vec::new();
    let mut totals: Vec<f32> = Vec::new();

    for m in measures {
        let n_events = m.sync_links.len();
        if n_events == 0 {
            positions.push(vec![]);
            totals.push(min_measure_width_stdist(0) as f32);
            continue;
        }

        let mut space_info: Vec<SpaceTimeInfo> = Vec::with_capacity(n_events);
        for ei in 0..n_events {
            let note_infos = &m.sync_note_infos[ei];
            let controlling_pdur = if !note_infos.is_empty() {
                let mut min_pdur = i32::MAX;
                for &(dur_code, dots, _, _) in note_infos {
                    if dur_code <= 0 {
                        continue;
                    }
                    let pdur = code_to_l_dur(dur_code, dots);
                    if pdur > 0 && pdur < min_pdur {
                        min_pdur = pdur;
                    }
                }
                if min_pdur == i32::MAX {
                    code_to_l_dur(4, 0)
                } else {
                    min_pdur
                }
            } else {
                code_to_l_dur(4, 0)
            };

            let time_to_next = if ei + 1 < n_events {
                (m.sync_times[ei + 1] - m.sync_times[ei]) as f32
            } else {
                controlling_pdur as f32
            };

            let frac = if controlling_pdur > 0 {
                (time_to_next / controlling_pdur as f32).clamp(0.0, 1.0)
            } else {
                1.0
            };

            let width_infos: Vec<NoteWidthInfo> = note_infos
                .iter()
                .map(|&(dur_code, dots, is_rest, acc)| NoteWidthInfo {
                    l_dur: dur_code,
                    ndots: dots,
                    rest: is_rest,
                    beamed: dur_code >= 4,
                    stem_up: true,
                    x_move_dots: if dur_code <= 2 { 5 } else { 3 },
                    acc,
                    xmove_acc: DFLT_XMOVEACC as u8,
                    courtesy_acc: false,
                    note_to_left: false,
                    note_to_right: false,
                })
                .collect();

            let w_right = sync_width_right(&width_infos, false);
            let w_left = sync_width_left(&width_infos);

            space_info.push(SpaceTimeInfo {
                index: ei,
                start_time: m.sync_times[ei] - m.sync_times.first().copied().unwrap_or(0),
                dur: controlling_pdur,
                frac,
                is_sync: true,
                just_type: J_IT,
                width_left: w_left,
                width_right: w_right,
            });
        }

        let pos = respace_1bar(&space_info, space_prop, 0);
        let total = if let Some(&last_pos) = pos.last() {
            let last_wr = space_info.last().map_or(0, |s| s.width_right);
            (last_pos + last_wr + CONFIG_SP_AFTER_BAR) as f32
        } else {
            min_measure_width_stdist(0) as f32
        };
        let total = total.max(min_measure_width_stdist(0) as f32);

        positions.push(pos);
        totals.push(total);
    }

    (positions, totals)
}

// ============================================================================
// System grouping
// ============================================================================

fn group_measures_into_systems(
    measure_widths: &[f32],
    config: &LayoutConfig,
    available_width: Ddist,
    continuation_available: Ddist,
) -> Vec<(usize, usize)> {
    let n = measure_widths.len();
    if n == 0 {
        return vec![];
    }

    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut start = 0;

    while start < n {
        let sys_avail = if ranges.is_empty() {
            available_width
        } else {
            continuation_available
        };
        let sys_avail_f = sys_avail as f32;

        let max_per_sys = if config.max_measures > 0 {
            config.max_measures
        } else {
            n
        };

        let mut end = start + 1;
        let mut total: f32 = 0.0;

        #[allow(clippy::needless_range_loop)]
        for mi in start..n.min(start + max_per_sys) {
            let w_ddist = stdist_to_ddist(measure_widths[mi], config.staff_height) as f32;
            if total + w_ddist > sys_avail_f && mi > start {
                break;
            }
            total += w_ddist;
            end = mi + 1;
        }

        ranges.push((start, end));
        start = end;
    }

    ranges
}

// ============================================================================
// Multi-system break insertion
// ============================================================================

/// Insert new System/Staff/Connect/Clef/KeySig objects at system boundaries.
/// Appends to `score.objects` and rewires the linked list.
fn insert_system_breaks(
    score: &mut InterpretedScore,
    config: &LayoutConfig,
    preamble: &PreambleInfo,
    measures: &[MeasureInfo],
    system_ranges: &[(usize, usize)],
    sys_per_page: usize,
) {
    if system_ranges.len() <= 1 {
        return;
    }

    // Find max subobject link in use
    let mut next_sub = find_max_sub_link(score) + 100;

    // Track all system links for l_system/r_system wiring
    let mut all_system_links = vec![preamble.system_link];

    for sys_idx in 1..system_ranges.len() {
        let (sys_start, _sys_end) = system_ranges[sys_idx];
        let sys_in_page = sys_idx % sys_per_page;
        let page_idx = sys_idx / sys_per_page;

        // Find the last object of the previous system
        let prev_sys_end = system_ranges[sys_idx - 1].1;
        let prev_last = measures[prev_sys_end - 1].last_link;

        // Insert PAGE at page boundary (page_idx > 0 means we need a new page)
        let splice_after = if sys_in_page == 0 && page_idx > 0 {
            let page_link = append_page_object(score, page_idx, prev_last, preamble.page_link);
            // Wire previous last → new page
            score.objects[prev_last as usize].header.right = page_link;
            page_link
        } else {
            prev_last
        };

        // Compute page_l for this system
        let page_l = if page_idx > 0 && sys_in_page == 0 {
            splice_after // the new page we just created
        } else {
            preamble.page_link
        };

        // Create new System + preamble objects
        let (sys_link, preamble_last) = append_system_preamble(
            score,
            config,
            preamble,
            &mut next_sub,
            sys_idx,
            sys_in_page,
            page_l,
        );
        all_system_links.push(sys_link);

        // Wire: splice_after → new system
        score.objects[splice_after as usize].header.right = sys_link;
        score.objects[sys_link as usize].header.left = splice_after;

        // Wire: preamble last → first measure of this system
        let first_meas = measures[sys_start].obj_link;
        score.objects[preamble_last as usize].header.right = first_meas;
        score.objects[first_meas as usize].header.left = preamble_last;
    }

    // Wire last system's last measure → TAIL
    let last_sys_end = system_ranges.last().unwrap().1;
    let last_obj = measures[last_sys_end - 1].last_link;
    score.objects[last_obj as usize].header.right = preamble.tail_link;
    score.objects[preamble.tail_link as usize].header.left = last_obj;

    // Wire l_system/r_system links
    for (i, &sys_link) in all_system_links.iter().enumerate() {
        if let ObjData::System(ref mut sys) = score.objects[sys_link as usize].data {
            sys.l_system = if i > 0 {
                all_system_links[i - 1]
            } else {
                NILINK
            };
            sys.r_system = if i + 1 < all_system_links.len() {
                all_system_links[i + 1]
            } else {
                NILINK
            };
            sys.system_num = (i + 1) as i16;
        }
    }
}

/// Append a PAGE object for a new page. Returns the new page's link.
fn append_page_object(
    score: &mut InterpretedScore,
    page_idx: usize,
    prev_link: Link,
    first_page_link: Link,
) -> Link {
    let page_link = score.objects.len() as Link;
    score.objects.push(InterpretedObject {
        index: page_link,
        header: ObjectHeader {
            obj_type: 4, // PAGE_TYPE
            left: prev_link,
            visible: true,
            valid: true,
            ..Default::default()
        },
        data: ObjData::Page(Page {
            header: ObjectHeader::default(),
            l_page: if page_idx > 1 {
                // Previous page link would need tracking; approximate with first_page_link
                first_page_link
            } else {
                first_page_link
            },
            r_page: NILINK, // Will be updated if more pages follow
            sheet_num: page_idx as i16,
            header_str_offset: 0,
            footer_str_offset: 0,
        }),
    });
    // Update first page's r_page to point to this page (if it was NILINK)
    if let ObjData::Page(ref mut p) = score.objects[first_page_link as usize].data {
        if p.r_page == NILINK {
            p.r_page = page_link;
        }
    }
    page_link
}

/// Append System + Staff + Connect + Clef + KeySig objects for a new system.
/// Returns (system_link, last_preamble_link).
fn append_system_preamble(
    score: &mut InterpretedScore,
    config: &LayoutConfig,
    preamble: &PreambleInfo,
    next_sub: &mut Link,
    sys_idx: usize,
    sys_in_page: usize,
    page_link: Link,
) -> (Link, Link) {
    let ns = preamble.num_staves;
    let dls = config.d_line_sp();
    let (ledger_w, notehead_w, frac_beam_w, flag_lead) = config.staff_metrics();
    let content_width = config.content_width();
    let system_height = config.staff_height + (ns as Ddist - 1) * config.inter_staff;
    let sys_top_i32 = config.system_top as i32 + sys_in_page as i32 * config.inter_system as i32;
    let sys_top = sys_top_i32.clamp(Ddist::MIN as i32, Ddist::MAX as i32) as Ddist;
    let sys_bottom =
        (sys_top_i32 + system_height as i32).clamp(Ddist::MIN as i32, Ddist::MAX as i32) as Ddist;

    // --- SYSTEM ---
    let system_link = score.objects.len() as Link;
    score.objects.push(InterpretedObject {
        index: system_link,
        header: ObjectHeader {
            obj_type: 5,
            visible: true,
            valid: true,
            ..Default::default()
        },
        data: ObjData::System(System {
            header: ObjectHeader::default(),
            l_system: NILINK,
            r_system: NILINK,
            page_l: page_link,
            system_num: (sys_idx + 1) as i16,
            system_rect: DRect {
                top: sys_top,
                left: config.system_left,
                bottom: sys_bottom,
                right: config.system_right,
            },
            sys_desc_ptr: 0,
        }),
    });

    // --- STAFF ---
    let staff_sub = *next_sub;
    *next_sub += ns as Link;
    let mut staff_subs: Vec<AStaff> = Vec::with_capacity(ns);
    for s in 0..ns {
        staff_subs.push(AStaff {
            next: if s + 1 < ns {
                staff_sub + (s + 1) as Link
            } else {
                NILINK
            },
            staffn: (s + 1) as i8,
            selected: false,
            visible: true,
            filler_stf: false,
            staff_top: (s as Ddist) * config.inter_staff,
            staff_left: 0,
            staff_right: content_width,
            staff_height: config.staff_height,
            staff_lines: 5,
            font_size: 24,
            flag_leading: flag_lead,
            min_stem_free: 0,
            ledger_width: ledger_w,
            note_head_width: notehead_w,
            frac_beam_width: frac_beam_w,
            space_below: config.inter_staff - config.staff_height,
            clef_type: preamble.clef_types.get(s).copied().unwrap_or(1),
            dynamic_type: 0,
            ks_info: preamble.ks_info,
            time_sig_type: 1,
            numerator: preamble.time_num,
            denominator: preamble.time_denom,
            filler: 0,
            show_ledgers: 1,
            show_lines: SHOW_ALL_LINES,
        });
    }
    let staff_link = score.objects.len() as Link;
    score.objects.push(InterpretedObject {
        index: staff_link,
        header: ObjectHeader {
            obj_type: 6,
            left: system_link,
            first_sub_obj: staff_sub,
            n_entries: ns as u8,
            visible: true,
            valid: true,
            ..Default::default()
        },
        data: ObjData::Staff(Staff {
            header: ObjectHeader::default(),
            l_staff: NILINK,
            r_staff: NILINK,
            system_l: system_link,
        }),
    });
    score.staffs.insert(staff_sub, staff_subs);
    score.objects[system_link as usize].header.right = staff_link;
    let mut prev_link = staff_link;

    // --- CONNECT (if multi-staff) ---
    if preamble.has_connect {
        let conn_sub = *next_sub;
        *next_sub += 1;
        let connect_link = score.objects.len() as Link;
        score.objects.push(InterpretedObject {
            index: connect_link,
            header: ObjectHeader {
                obj_type: 12,
                left: prev_link,
                first_sub_obj: conn_sub,
                n_entries: 1,
                visible: true,
                valid: true,
                ..Default::default()
            },
            data: ObjData::Connect(Connect {
                header: ObjectHeader::default(),
                conn_filler: NILINK,
            }),
        });
        score.connects.insert(
            conn_sub,
            vec![AConnect {
                next: NILINK,
                selected: false,
                filler: 0,
                conn_level: 0,
                connect_type: preamble.connect_type,
                staff_above: preamble.staff_above,
                staff_below: preamble.staff_below,
                xd: 0,
                first_part: 1,
                last_part: 1,
            }],
        );
        score.objects[prev_link as usize].header.right = connect_link;
        prev_link = connect_link;
    }

    // --- CLEF (continuation) ---
    let clef_sub = *next_sub;
    *next_sub += ns as Link;
    let clef_xd = dls;
    let mut clef_subs: Vec<AClef> = Vec::with_capacity(ns);
    for s in 0..ns {
        clef_subs.push(AClef {
            header: SubObjHeader {
                next: if s + 1 < ns {
                    clef_sub + (s + 1) as Link
                } else {
                    NILINK
                },
                staffn: (s + 1) as i8,
                sub_type: preamble.clef_types.get(s).copied().unwrap_or(1),
                selected: false,
                visible: true,
                soft: false,
            },
            filler1: 0,
            small: 0,
            filler2: 0,
            xd: 0,
            yd: 0,
        });
    }
    let clef_link = score.objects.len() as Link;
    score.objects.push(InterpretedObject {
        index: clef_link,
        header: ObjectHeader {
            obj_type: 8,
            left: prev_link,
            first_sub_obj: clef_sub,
            n_entries: ns as u8,
            xd: clef_xd,
            visible: true,
            valid: true,
            ..Default::default()
        },
        data: ObjData::Clef(Clef {
            header: ObjectHeader::default(),
            in_measure: false,
        }),
    });
    score.clefs.insert(clef_sub, clef_subs);
    score.objects[prev_link as usize].header.right = clef_link;
    prev_link = clef_link;

    // --- KEYSIG (continuation) ---
    let ks_sub = *next_sub;
    *next_sub += ns as Link;
    let n_ks = preamble.ks_info.n_ks_items as i32;
    let ks_width: Ddist = if n_ks > 0 {
        stdist_to_ddist(9.0 * n_ks as f32, config.staff_height) + dls
    } else {
        0
    };
    let keysig_xd = clef_xd + (5 * dls) / 2;
    let _ = keysig_xd + ks_width; // total preamble (unused here, used in assign_xd)

    let mut ks_subs: Vec<AKeySig> = Vec::with_capacity(ns);
    for s in 0..ns {
        ks_subs.push(AKeySig {
            header: SubObjHeader {
                next: if s + 1 < ns {
                    ks_sub + (s + 1) as Link
                } else {
                    NILINK
                },
                staffn: (s + 1) as i8,
                sub_type: 0,
                selected: false,
                visible: true,
                soft: false,
            },
            nonstandard: 0,
            filler1: 0,
            small: 0,
            filler2: 0,
            xd: 0,
            ks_info: preamble.ks_info,
        });
    }
    let ks_link = score.objects.len() as Link;
    score.objects.push(InterpretedObject {
        index: ks_link,
        header: ObjectHeader {
            obj_type: 9,
            left: prev_link,
            first_sub_obj: ks_sub,
            n_entries: ns as u8,
            xd: keysig_xd,
            visible: true,
            valid: true,
            ..Default::default()
        },
        data: ObjData::KeySig(KeySig {
            header: ObjectHeader::default(),
            in_measure: false,
        }),
    });
    score.keysigs.insert(ks_sub, ks_subs);
    score.objects[prev_link as usize].header.right = ks_link;

    (system_link, ks_link)
}

// ============================================================================
// xd position assignment
// ============================================================================

#[allow(clippy::too_many_arguments)]
fn assign_xd_positions(
    score: &mut InterpretedScore,
    measures: &[MeasureInfo],
    system_ranges: &[(usize, usize)],
    positions_stdist: &[Vec<i16>],
    totals_stdist: &[f32],
    config: &LayoutConfig,
    preamble_width: Ddist,
    cont_preamble: Ddist,
    available: Ddist,
    cont_available: Ddist,
) {
    let mut measure_abs_xd: Vec<Ddist> = Vec::new();
    let mut measure_width_ddist: Vec<Ddist> = Vec::new();

    for (sys_idx, &(sys_start, sys_end)) in system_ranges.iter().enumerate() {
        let sys_preamble = if sys_idx == 0 {
            preamble_width
        } else {
            cont_preamble
        };
        let sys_available = if sys_idx == 0 {
            available
        } else {
            cont_available
        };

        let sys_ideal: f32 = totals_stdist[sys_start..sys_end].iter().sum();
        let sys_ideal_ddist = stdist_to_ddist(sys_ideal, config.staff_height);

        let sys_scale = if sys_ideal_ddist > 0 {
            sys_available as f32 / sys_ideal_ddist as f32
        } else {
            1.0
        };

        let mut x_cursor: Ddist = sys_preamble;
        for &mt in &totals_stdist[sys_start..sys_end] {
            measure_abs_xd.push(x_cursor);
            let w = (stdist_to_ddist(mt, config.staff_height) as f32 * sys_scale) as Ddist;
            let w = w.max(200);
            measure_width_ddist.push(w);
            x_cursor += w;
        }
    }

    for (mi, m) in measures.iter().enumerate() {
        if mi >= measure_abs_xd.len() {
            break;
        }
        score.objects[m.obj_link as usize].header.xd = measure_abs_xd[mi];

        let actual_width = measure_width_ddist[mi];
        let ideal_total = totals_stdist[mi];
        let positions = &positions_stdist[mi];

        let ideal_ddist = stdist_to_ddist(ideal_total, config.staff_height);
        let inner_scale = if ideal_ddist > 0 {
            actual_width as f32 / ideal_ddist as f32
        } else {
            1.0
        };

        for (ei, &sync_link) in m.sync_links.iter().enumerate() {
            let stdist_pos = positions.get(ei).copied().unwrap_or(0) as f32;
            let raw_ddist = stdist_to_ddist(stdist_pos, config.staff_height);
            let rel = (raw_ddist as f32 * inner_scale) as Ddist;
            score.objects[sync_link as usize].header.xd = rel;
        }
    }
}

// ============================================================================
// Geometry fixup helpers
// ============================================================================

/// Fix staff geometry for ALL existing staff entries.
fn fix_staff_geometry(score: &mut InterpretedScore, config: &LayoutConfig) {
    let (ledger_w, notehead_w, frac_beam_w, flag_lead) = config.staff_metrics();
    let content_width = config.content_width();

    for staffs in score.staffs.values_mut() {
        for (i, staff) in staffs.iter_mut().enumerate() {
            staff.staff_top = (i as Ddist) * config.inter_staff;
            staff.staff_left = 0;
            staff.staff_right = content_width;
            staff.staff_height = config.staff_height;
            staff.ledger_width = ledger_w;
            staff.note_head_width = notehead_w;
            staff.frac_beam_width = frac_beam_w;
            staff.flag_leading = flag_lead;
            staff.space_below = config.inter_staff - config.staff_height;
        }
    }
}

/// Fix system_rect for a system at a given page-relative position.
fn fix_system_geometry_at(
    score: &mut InterpretedScore,
    config: &LayoutConfig,
    preamble: &PreambleInfo,
    sys_in_page: usize,
) {
    let ns = preamble.num_staves;
    let system_height = config.staff_height + (ns as Ddist - 1) * config.inter_staff;
    let sys_top = config.system_top + sys_in_page as Ddist * config.inter_system;
    let sys_bottom = sys_top + system_height;

    // Fix only the first (original) system object
    if let ObjData::System(ref mut sys) = score.objects[preamble.system_link as usize].data {
        sys.system_rect = DRect {
            top: sys_top,
            left: config.system_left,
            bottom: sys_bottom,
            right: config.system_right,
        };
    }
}

/// Fix preamble xd positions for the first system's Clef/KeySig/TimeSig.
fn fix_preamble_positions(score: &mut InterpretedScore, config: &LayoutConfig) {
    let dls = config.d_line_sp();
    let clef_xd = dls;

    let max_ks_items = score
        .keysigs
        .values()
        .flat_map(|v| v.iter())
        .map(|ks| ks.ks_info.n_ks_items as i32)
        .max()
        .unwrap_or(0);

    let ks_width: Ddist = if max_ks_items > 0 {
        stdist_to_ddist(9.0 * max_ks_items as f32, config.staff_height) + dls
    } else {
        0
    };

    let keysig_xd = clef_xd + (7 * dls) / 2;
    let timesig_xd = keysig_xd + ks_width;

    // Only fix preamble objects that exist before the first Measure
    let mut past_first_measure = false;
    for obj in score.objects.iter_mut() {
        if matches!(obj.data, ObjData::Measure(_)) {
            past_first_measure = true;
        }
        if past_first_measure {
            break;
        }
        match &obj.data {
            ObjData::Clef(clef) => {
                if !clef.in_measure {
                    obj.header.xd = clef_xd;
                }
            }
            ObjData::KeySig(ks) => {
                if !ks.in_measure {
                    obj.header.xd = keysig_xd;
                }
            }
            ObjData::TimeSig(_) => {
                obj.header.xd = timesig_xd;
            }
            _ => {}
        }
    }
}

// ============================================================================
// Preamble width computation (from PreambleInfo, not score)
// ============================================================================

fn compute_preamble_width_info(preamble: &PreambleInfo, config: &LayoutConfig) -> Ddist {
    let dls = config.d_line_sp();
    let clef_xd = dls;
    let n_ks = preamble.ks_info.n_ks_items as i32;
    let ks_width: Ddist = if n_ks > 0 {
        stdist_to_ddist(9.0 * n_ks as f32, config.staff_height) + dls
    } else {
        0
    };
    let keysig_xd = clef_xd + (7 * dls) / 2;
    let timesig_xd = keysig_xd + ks_width;
    timesig_xd + 3 * dls
}

fn compute_continuation_preamble_info(preamble: &PreambleInfo, config: &LayoutConfig) -> Ddist {
    let dls = config.d_line_sp();
    let clef_xd = dls;
    let n_ks = preamble.ks_info.n_ks_items as i32;
    let ks_width: Ddist = if n_ks > 0 {
        stdist_to_ddist(9.0 * n_ks as f32, config.staff_height) + dls
    } else {
        0
    };
    clef_xd + (5 * dls) / 2 + ks_width
}

// ============================================================================
// Utility
// ============================================================================

/// Find the maximum subobject link key across all HashMap stores.
fn find_max_sub_link(score: &InterpretedScore) -> Link {
    let mut max_link: Link = 0;
    for (&key, subs) in &score.staffs {
        max_link = max_link.max(key + subs.len() as Link);
    }
    for (&key, subs) in &score.clefs {
        max_link = max_link.max(key + subs.len() as Link);
    }
    for (&key, subs) in &score.keysigs {
        max_link = max_link.max(key + subs.len() as Link);
    }
    for (&key, subs) in &score.timesigs {
        max_link = max_link.max(key + subs.len() as Link);
    }
    for (&key, subs) in &score.connects {
        max_link = max_link.max(key + subs.len() as Link);
    }
    for (&key, subs) in &score.notes {
        max_link = max_link.max(key + subs.len() as Link);
    }
    for (&key, subs) in &score.notebeams {
        max_link = max_link.max(key + subs.len() as Link);
    }
    for (&key, subs) in &score.dynamics {
        max_link = max_link.max(key + subs.len() as Link);
    }
    for (&key, subs) in &score.measures {
        max_link = max_link.max(key + subs.len() as Link);
    }
    for (&key, subs) in &score.slurs {
        max_link = max_link.max(key + subs.len() as Link);
    }
    max_link
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_config_defaults() {
        let c = LayoutConfig::default();
        assert_eq!(c.page_width, 612);
        assert_eq!(c.page_height, 792);
        assert_eq!(c.staff_height, 384);
        assert_eq!(c.system_left, 1152); // 72pt * 16
        assert_eq!(c.system_right, 8928); // (612-54) * 16
        assert!(c.content_width() > 0);
    }

    #[test]
    fn test_preamble_width_positive() {
        let preamble = PreambleInfo {
            num_staves: 1,
            clef_types: vec![1],
            ks_info: KsInfo::default(),
            time_num: 4,
            time_denom: 4,
            has_connect: false,
            connect_type: 3,
            staff_above: 1,
            staff_below: 1,
            system_link: NILINK,
            page_link: NILINK,
            tail_link: NILINK,
        };
        let c = LayoutConfig::default();
        let pw = compute_preamble_width_info(&preamble, &c);
        assert!(pw > 0, "Preamble width should be positive: {}", pw);
    }

    #[test]
    fn test_systems_per_page() {
        let c = LayoutConfig::default();
        let spp = c.systems_per_page(2);
        assert!((1..=10).contains(&spp), "Reasonable systems/page: {}", spp);
    }
}
