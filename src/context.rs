//! Musical context system: propagates clef, key signature, time signature, and dynamic
//! information through the score.
//!
//! Ported from:
//! - `Nightingale/src/CFilesBoth/Context.cp`
//!
//! In Nightingale, a "context" is the set of musical attributes (clef, key sig, time sig,
//! dynamic) that are in effect at any point in the score. As you traverse the object list
//! from left to right:
//! - A **STAFF** object sets initial context for each staff (staff geometry + stored context fields)
//! - A **MEASURE** object updates measure visibility and position, and carries stored context
//! - A **CLEF** object changes the clef for the staff it's on
//! - A **KEYSIG** object changes the key signature
//! - A **TIMESIG** object changes the time signature
//! - A **DYNAMIC** object changes the dynamic level
//!
//! The key C++ function `GetContext(doc, contextL, theStaff, pContext)` searches backwards
//! from a given object to find what clef/key/timesig/dynamic is in effect. This Rust
//! implementation uses forward traversal instead, since we have the full object list in memory.

use crate::basic_types::{KsInfo, Link, Rect};
use crate::defs::*;
use crate::ngl::interpret::{InterpretedObject, InterpretedScore, ObjData};
use crate::obj_types::{Context, SHOW_ALL_LINES};

/// ContextState: holds context for all staves at a specific point in the score.
///
/// The contexts vector uses 1-based indexing (index 0 is unused) to match Nightingale's
/// staff numbering convention.
#[derive(Debug, Clone)]
pub struct ContextState {
    /// Context for each staff (1-based indexing, index 0 unused)
    contexts: Vec<Context>,
    num_staves: usize,
}

impl ContextState {
    /// Create a new ContextState with default context for all staves.
    ///
    /// # Arguments
    /// * `num_staves` - Number of staves (1-based, so we allocate num_staves+1 slots)
    ///
    /// # Returns
    /// A new ContextState with all staves initialized to default values.
    pub fn new(num_staves: usize) -> Self {
        let mut contexts = Vec::with_capacity(num_staves + 1);

        // Slot 0 is unused (to match 1-based staff numbering)
        contexts.push(Self::default_context());

        // Initialize contexts for staves 1..=num_staves
        for _ in 1..=num_staves {
            contexts.push(Self::default_context());
        }

        Self {
            contexts,
            num_staves,
        }
    }

    /// Get the context for a specific staff.
    ///
    /// # Arguments
    /// * `staff` - Staff number (1-based)
    ///
    /// # Returns
    /// Reference to the context, or None if staff number is out of range.
    pub fn get(&self, staff: i8) -> Option<&Context> {
        if staff < 1 || staff as usize > self.num_staves {
            return None;
        }
        self.contexts.get(staff as usize)
    }

    /// Get a mutable reference to the context for a specific staff.
    ///
    /// # Arguments
    /// * `staff` - Staff number (1-based)
    ///
    /// # Returns
    /// Mutable reference to the context, or None if staff number is out of range.
    pub fn get_mut(&mut self, staff: i8) -> Option<&mut Context> {
        if staff < 1 || staff as usize > self.num_staves {
            return None;
        }
        self.contexts.get_mut(staff as usize)
    }

    /// Update context based on an object from the score.
    ///
    /// This method examines the object type and updates the appropriate context fields
    /// for the relevant staves. It handles:
    /// - System objects: update systemTop/Left/Bottom for all staves
    /// - Staff objects: update staff geometry and stored context per staff
    /// - Measure objects: update measure visibility and position per staff
    /// - Clef objects: update clefType per staff
    /// - KeySig objects: update ksInfo per staff
    /// - TimeSig objects: update numerator/denominator per staff
    /// - Dynamic objects: update dynamicType per staff
    ///
    /// # Arguments
    /// * `obj` - The object to process
    /// * `score` - The full score (for accessing subobjects)
    pub fn update_from_object(&mut self, obj: &InterpretedObject, score: &InterpretedScore) {
        match &obj.data {
            ObjData::System(system) => {
                // System object updates system geometry for all staves
                for staff in 1..=self.num_staves {
                    if let Some(ctx) = self.get_mut(staff as i8) {
                        ctx.system_num = system.system_num;
                        ctx.system_top = system.system_rect.top;
                        ctx.system_left = system.system_rect.left;
                        ctx.system_bottom = system.system_rect.bottom;
                    }
                }
            }

            ObjData::Staff(_staff) => {
                // Staff object: iterate subobjects and update staff geometry + context
                if let Some(astaff_list) = score.staffs.get(&obj.header.first_sub_obj) {
                    for astaff in astaff_list {
                        if let Some(ctx) = self.get_mut(astaff.staffn) {
                            // Update geometry
                            ctx.staff_visible = astaff.visible;
                            ctx.staff_top = astaff.staff_top;
                            ctx.staff_left = astaff.staff_left;
                            ctx.staff_right = astaff.staff_right;
                            ctx.staff_height = astaff.staff_height;
                            ctx.staff_half_height = astaff.staff_height / 2;
                            ctx.staff_lines = astaff.staff_lines;
                            ctx.show_lines = astaff.show_lines as i8;
                            ctx.show_ledgers = astaff.show_ledgers != 0;
                            ctx.font_size = astaff.font_size;

                            // Update stored context from AStaff
                            ctx.clef_type = astaff.clef_type;
                            ctx.dynamic_type = astaff.dynamic_type;
                            ctx.ks_info = astaff.ks_info;
                            ctx.time_sig_type = astaff.time_sig_type;
                            ctx.numerator = astaff.numerator;
                            ctx.denominator = astaff.denominator;

                            // Update visibility
                            ctx.visible = ctx.staff_visible && ctx.measure_visible;
                        }
                    }
                }
            }

            ObjData::Measure(_measure) => {
                // Measure object: iterate subobjects and update measure visibility + position
                if let Some(ameasure_list) = score.measures.get(&obj.header.first_sub_obj) {
                    for ameasure in ameasure_list {
                        if let Some(ctx) = self.get_mut(ameasure.header.staffn) {
                            ctx.in_measure = true;
                            ctx.measure_visible = ameasure.measure_visible;
                            ctx.measure_top = ameasure.meas_size_rect.top;
                            ctx.measure_left = obj.header.xd; // Measure xd is the left edge

                            // Update stored context from AMeasure
                            ctx.clef_type = ameasure.clef_type;
                            ctx.dynamic_type = ameasure.dynamic_type;
                            ctx.ks_info = ameasure.ks_info;
                            ctx.time_sig_type = ameasure.time_sig_type;
                            ctx.numerator = ameasure.numerator;
                            ctx.denominator = ameasure.denominator;

                            // Update visibility
                            ctx.visible = ctx.staff_visible && ctx.measure_visible;
                        }
                    }
                }
            }

            ObjData::Clef(_clef) => {
                // Clef object: iterate subobjects and update clefType per staff
                if let Some(aclef_list) = score.clefs.get(&obj.header.first_sub_obj) {
                    for aclef in aclef_list {
                        if let Some(ctx) = self.get_mut(aclef.header.staffn) {
                            ctx.clef_type = aclef.header.sub_type;
                        }
                    }
                }
            }

            ObjData::KeySig(_keysig) => {
                // KeySig object: iterate subobjects and update ksInfo per staff
                if let Some(akeysig_list) = score.keysigs.get(&obj.header.first_sub_obj) {
                    for akeysig in akeysig_list {
                        if let Some(ctx) = self.get_mut(akeysig.header.staffn) {
                            ctx.ks_info = akeysig.ks_info;
                        }
                    }
                }
            }

            ObjData::TimeSig(timesig) => {
                // TimeSig object: iterate subobjects and update numerator/denominator per staff
                if let Some(atimesig_list) = score.timesigs.get(&obj.header.first_sub_obj) {
                    for atimesig in atimesig_list {
                        if let Some(ctx) = self.get_mut(atimesig.header.staffn) {
                            ctx.time_sig_type = timesig.header.obj_type;
                            ctx.numerator = atimesig.numerator;
                            ctx.denominator = atimesig.denominator;
                        }
                    }
                }
            }

            ObjData::Dynamic(_dynamic) => {
                // Dynamic object: iterate subobjects and update dynamicType per staff
                if let Some(adynamic_list) = score.dynamics.get(&obj.header.first_sub_obj) {
                    for adynamic in adynamic_list {
                        if let Some(ctx) = self.get_mut(adynamic.header.staffn) {
                            ctx.dynamic_type = adynamic.header.sub_type;
                        }
                    }
                }
            }

            _ => {
                // Other object types don't affect context
            }
        }
    }

    /// Create a default context with standard values.
    ///
    /// Uses the DFLT_* constants from defs.rs:
    /// - DFLT_CLEF (3 = treble)
    /// - DFLT_NKSITEMS (0 = C major/A minor)
    /// - DFLT_TSTYPE (1 = N_OVER_D)
    /// - DFLT_NUMER (4)
    /// - DFLT_DENOM (4)
    /// - DFLT_DYNAMIC (6 = mf)
    fn default_context() -> Context {
        Context {
            visible: true,
            staff_visible: true,
            measure_visible: true,
            in_measure: false,
            paper: Rect {
                top: 0,
                left: 0,
                bottom: 0,
                right: 0,
            },
            sheet_num: 0,
            system_num: 0,
            system_top: 0,
            system_left: 0,
            system_bottom: 0,
            staff_top: 0,
            staff_left: 0,
            staff_right: 0,
            staff_height: 0,
            staff_half_height: 0,
            staff_lines: 5,
            show_lines: SHOW_ALL_LINES as i8,
            show_ledgers: true,
            font_size: 0,
            measure_top: 0,
            measure_left: 0,
            clef_type: DFLT_CLEF as i8,
            dynamic_type: DFLT_DYNAMIC as i8,
            ks_info: KsInfo::default(),
            time_sig_type: DFLT_TSTYPE,
            numerator: DFLT_NUMER,
            denominator: DFLT_DENOM,
        }
    }
}

/// Get the context at a specific object for a specific staff.
///
/// This is the Rust equivalent of C++ GetContext(doc, contextL, theStaff, pContext).
/// Instead of searching backward, we walk forward from the beginning of the score,
/// updating context as we go, and return the context at the target object.
///
/// # Arguments
/// * `score` - The full score
/// * `target_link` - The object link to get context at
/// * `staff` - The staff number (1-based)
///
/// # Returns
/// The context at the target object for the given staff. If target_link is not found,
/// returns the context at the end of the score.
pub fn get_context_at(score: &InterpretedScore, target_link: Link, staff: i8) -> Context {
    // Determine number of staves from the first Staff object
    let num_staves = count_staves(score);
    let mut state = ContextState::new(num_staves);

    // Walk the score from the beginning, updating context as we go
    for obj in score.walk() {
        // If we've reached the target, return the context
        if obj.index == target_link {
            return state
                .get(staff)
                .cloned()
                .unwrap_or_else(ContextState::default_context);
        }

        // Update context based on this object
        state.update_from_object(obj, score);
    }

    // If target_link wasn't found, return the current context
    state
        .get(staff)
        .cloned()
        .unwrap_or_else(ContextState::default_context)
}

/// Build a context map for the entire score.
///
/// This walks the entire score once, building a snapshot of context state at each object.
/// This is useful for batch operations that need context at multiple points.
///
/// # Arguments
/// * `score` - The full score
/// * `num_staves` - Number of staves in the score
///
/// # Returns
/// A vector of (Link, ContextState) pairs, one for each object in the score.
pub fn build_context_map(score: &InterpretedScore, num_staves: usize) -> Vec<(Link, ContextState)> {
    let mut map = Vec::new();
    let mut state = ContextState::new(num_staves);

    for obj in score.walk() {
        // Update context based on this object
        state.update_from_object(obj, score);

        // Store a snapshot of the current state
        map.push((obj.index, state.clone()));
    }

    map
}

/// Count the number of staves in a score by examining the first Staff object.
///
/// # Arguments
/// * `score` - The full score
///
/// # Returns
/// The number of staves, or 1 if no Staff object is found.
fn count_staves(score: &InterpretedScore) -> usize {
    for obj in score.walk() {
        if let ObjData::Staff(_) = &obj.data {
            if let Some(astaff_list) = score.staffs.get(&obj.header.first_sub_obj) {
                return astaff_list.len();
            }
        }
    }
    1 // Default to 1 staff if we can't find any
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basic_types::{DRect, NILINK};
    use crate::obj_types::{
        AClef, AKeySig, ATimeSig, Clef, KeySig, ObjectHeader, SubObjHeader, System, TimeSig,
    };

    /// Create a basic ObjectHeader for testing
    fn make_header(index: Link, obj_type: u8) -> ObjectHeader {
        ObjectHeader {
            right: if index == 1 { NILINK } else { index + 1 },
            left: if index == 1 { NILINK } else { index - 1 },
            first_sub_obj: NILINK,
            xd: 0,
            yd: 0,
            obj_type: obj_type as i8,
            selected: false,
            visible: true,
            soft: false,
            valid: true,
            tweaked: false,
            spare_flag: false,
            ohdr_filler1: 0,
            obj_rect: Rect {
                top: 0,
                left: 0,
                bottom: 100,
                right: 100,
            },
            rel_size: 0,
            ohdr_filler2: 0,
            n_entries: 1,
        }
    }

    #[test]
    fn test_context_state_new() {
        let state = ContextState::new(2);

        // Check that we have 3 contexts (slot 0 + 2 staves)
        assert_eq!(state.contexts.len(), 3);
        assert_eq!(state.num_staves, 2);

        // Check that staves 1 and 2 have default context
        let ctx1 = state.get(1).unwrap();
        assert_eq!(ctx1.clef_type, DFLT_CLEF as i8);
        assert_eq!(ctx1.numerator, DFLT_NUMER);
        assert_eq!(ctx1.denominator, DFLT_DENOM);
        assert_eq!(ctx1.dynamic_type, DFLT_DYNAMIC as i8);

        let ctx2 = state.get(2).unwrap();
        assert_eq!(ctx2.clef_type, DFLT_CLEF as i8);

        // Check that out-of-range staff returns None
        assert!(state.get(0).is_none());
        assert!(state.get(3).is_none());
    }

    #[test]
    fn test_context_system_update() {
        let mut state = ContextState::new(2);
        let score = InterpretedScore::new();

        // Create a System object
        let system = System {
            header: make_header(1, SYSTEM_TYPE),
            l_system: NILINK,
            r_system: NILINK,
            page_l: NILINK,
            system_num: 1,
            system_rect: DRect {
                top: 100,
                left: 50,
                bottom: 500,
                right: 400,
            },
            sys_desc_ptr: 0,
        };

        let obj = InterpretedObject {
            index: 1,
            header: system.header.clone(),
            data: ObjData::System(system),
        };

        // Update context from System
        state.update_from_object(&obj, &score);

        // Check that both staves got updated with system geometry
        let ctx1 = state.get(1).unwrap();
        assert_eq!(ctx1.system_num, 1);
        assert_eq!(ctx1.system_top, 100);
        assert_eq!(ctx1.system_left, 50);
        assert_eq!(ctx1.system_bottom, 500);

        let ctx2 = state.get(2).unwrap();
        assert_eq!(ctx2.system_num, 1);
        assert_eq!(ctx2.system_top, 100);
    }

    #[test]
    fn test_context_clef_update() {
        let mut state = ContextState::new(2);
        let mut score = InterpretedScore::new();

        // Create a Clef object with subobjects
        let clef = Clef {
            header: make_header(2, CLEF_TYPE),
            in_measure: true,
        };

        // Create AClef subobjects
        let aclef1 = AClef {
            header: SubObjHeader {
                next: NILINK,
                staffn: 1,
                sub_type: BASS_CLEF as i8, // Change staff 1 to bass clef
                selected: false,
                visible: true,
                soft: false,
            },
            filler1: 0,
            small: 0,
            filler2: 0,
            xd: 0,
            yd: 0,
        };

        let aclef2 = AClef {
            header: SubObjHeader {
                next: NILINK,
                staffn: 2,
                sub_type: ALTO_CLEF as i8, // Change staff 2 to alto clef
                selected: false,
                visible: true,
                soft: false,
            },
            filler1: 0,
            small: 0,
            filler2: 0,
            xd: 0,
            yd: 0,
        };

        score
            .clefs
            .insert(clef.header.first_sub_obj, vec![aclef1, aclef2]);

        let obj = InterpretedObject {
            index: 2,
            header: clef.header.clone(),
            data: ObjData::Clef(clef),
        };

        // Update context from Clef
        state.update_from_object(&obj, &score);

        // Check that clefs were updated
        let ctx1 = state.get(1).unwrap();
        assert_eq!(ctx1.clef_type, BASS_CLEF as i8);

        let ctx2 = state.get(2).unwrap();
        assert_eq!(ctx2.clef_type, ALTO_CLEF as i8);
    }

    #[test]
    fn test_context_timesig_update() {
        let mut state = ContextState::new(1);
        let mut score = InterpretedScore::new();

        // Create a TimeSig object
        let mut header = make_header(3, TIMESIG_TYPE);
        header.first_sub_obj = 100; // Link to subobject

        let timesig = TimeSig {
            header: header.clone(),
            in_measure: true,
        };

        // Create ATimeSig subobject for 3/4 time
        let atimesig = ATimeSig {
            header: SubObjHeader {
                next: NILINK,
                staffn: 1,
                sub_type: 0,
                selected: false,
                visible: true,
                soft: false,
            },
            filler: 0,
            small: 0,
            conn_staff: 0,
            xd: 0,
            yd: 0,
            numerator: 3,
            denominator: 4,
        };

        score.timesigs.insert(100, vec![atimesig]);

        let obj = InterpretedObject {
            index: 3,
            header,
            data: ObjData::TimeSig(timesig),
        };

        // Update context from TimeSig
        state.update_from_object(&obj, &score);

        // Check that time signature was updated
        let ctx = state.get(1).unwrap();
        assert_eq!(ctx.numerator, 3);
        assert_eq!(ctx.denominator, 4);
    }

    #[test]
    fn test_context_keysig_update() {
        let mut state = ContextState::new(1);
        let mut score = InterpretedScore::new();

        // Create a KeySig object
        let mut header = make_header(4, KEYSIG_TYPE);
        header.first_sub_obj = 200;

        let keysig = KeySig {
            header: header.clone(),
            in_measure: false,
        };

        // Create AKeySig subobject with 2 sharps (D major)
        let ks_info = KsInfo {
            n_ks_items: 2,
            ..KsInfo::default()
        };

        let akeysig = AKeySig {
            header: SubObjHeader {
                next: NILINK,
                staffn: 1,
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
            ks_info,
        };

        score.keysigs.insert(200, vec![akeysig]);

        let obj = InterpretedObject {
            index: 4,
            header,
            data: ObjData::KeySig(keysig),
        };

        // Update context from KeySig
        state.update_from_object(&obj, &score);

        // Check that key signature was updated
        let ctx = state.get(1).unwrap();
        assert_eq!(ctx.ks_info.n_ks_items, 2);
    }

    #[test]
    fn test_context_propagation() {
        let mut state = ContextState::new(1);
        let mut score = InterpretedScore::new();

        // Start with default treble clef
        let ctx = state.get(1).unwrap();
        assert_eq!(ctx.clef_type, TREBLE_CLEF as i8);

        // Add a bass clef change
        let mut clef_header = make_header(5, CLEF_TYPE);
        clef_header.first_sub_obj = 300;

        let clef = Clef {
            header: clef_header.clone(),
            in_measure: true,
        };

        let aclef = AClef {
            header: SubObjHeader {
                next: NILINK,
                staffn: 1,
                sub_type: BASS_CLEF as i8,
                selected: false,
                visible: true,
                soft: false,
            },
            filler1: 0,
            small: 0,
            filler2: 0,
            xd: 0,
            yd: 0,
        };

        score.clefs.insert(300, vec![aclef]);

        let obj = InterpretedObject {
            index: 5,
            header: clef_header,
            data: ObjData::Clef(clef),
        };

        state.update_from_object(&obj, &score);

        // Clef should now be bass
        let ctx = state.get(1).unwrap();
        assert_eq!(ctx.clef_type, BASS_CLEF as i8);
    }
}
