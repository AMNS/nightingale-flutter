//! Stub unpackers for N105 subobject types not yet fully implemented.
//!
//! These will be filled in as we port additional rendering features.

use crate::obj_types::{AConnect, ADynamic, AGraphic, AModNr, ANoteOttava, APsMeas, ARptEnd};

pub fn unpack_aconnect_n105(_data: &[u8]) -> Result<AConnect, String> {
    // TODO: Implement full ACONNECT_5 unpacking (12 bytes, bitfields in byte 2)
    Err("ACONNECT unpacking not yet implemented".to_string())
}

pub fn unpack_adynamic_n105(_data: &[u8]) -> Result<ADynamic, String> {
    // TODO: Implement full ADYNAMIC_5 unpacking (12 bytes, bitfields in bytes 4-5)
    Err("ADYNAMIC unpacking not yet implemented".to_string())
}

pub fn unpack_amodnr_n105(_data: &[u8]) -> Result<AModNr, String> {
    // TODO: Implement full AMODNR_5 unpacking (6 bytes, bitfields in byte 4)
    Err("AMODNR unpacking not yet implemented".to_string())
}

pub fn unpack_agraphic_n105(_data: &[u8]) -> Result<AGraphic, String> {
    // TODO: Implement full AGRAPHIC_5 unpacking (6 bytes)
    Err("AGRAPHIC unpacking not yet implemented".to_string())
}

pub fn unpack_anoteottava_n105(_data: &[u8]) -> Result<ANoteOttava, String> {
    // TODO: Implement full ANOTEOTTAVA_5 unpacking (4 bytes)
    Err("ANOTEOTTAVA unpacking not yet implemented".to_string())
}

pub fn unpack_arptend_n105(_data: &[u8]) -> Result<ARptEnd, String> {
    // TODO: Implement full ARPTEND_5 unpacking (6 bytes, bitfields in byte 4)
    Err("ARPTEND unpacking not yet implemented".to_string())
}

pub fn unpack_apsmeas_n105(_data: &[u8]) -> Result<APsMeas, String> {
    // TODO: Implement full APSMEAS_5 unpacking (6 bytes, bitfields in byte 4)
    Err("APSMEAS unpacking not yet implemented".to_string())
}
