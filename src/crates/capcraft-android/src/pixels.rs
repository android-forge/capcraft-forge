// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 CapCraft contributors

//! A picture put straight into memory the app owns: handed over as bytes, a
//! preview frame is copied and allocated anew on either side of the bridge,
//! which cost more than drawing it. With [`crate::android`], the only unsafe
//! code in the crate.
#![allow(unsafe_code)]

use crate::{MobileError, Picture};

/// Copies `picture` to `address`, where the app holds a direct buffer of
/// `capacity` bytes.
pub(crate) fn put(picture: &Picture, address: u64, capacity: u64) -> Result<(), MobileError> {
    let length = picture.rgba.len();
    if address == 0 || u64::try_from(length).map_or(true, |length| length > capacity) {
        return Err(MobileError::Failed("no room for the picture".to_owned()));
    }
    let Ok(address) = usize::try_from(address) else {
        return Err(MobileError::Failed("not an address".to_owned()));
    };
    // SAFETY: the app passes the address of a direct buffer it keeps alive
    // and does not touch for the length of this call, and how large it is,
    // which the length was just held to; the picture's own bytes are another
    // allocation.
    unsafe {
        std::ptr::copy_nonoverlapping(picture.rgba.as_ptr(), address as *mut u8, length);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_is_put_where_there_is_room_and_nowhere_else() {
        let picture = Picture {
            width: 2,
            height: 1,
            rgba: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        let mut room = [0_u8; 9];
        let address = room.as_mut_ptr() as u64;
        put(&picture, address, 9).expect("there is room");
        assert_eq!(room, [1, 2, 3, 4, 5, 6, 7, 8, 0]);
        assert!(put(&picture, address, 7).is_err());
        assert!(put(&picture, 0, 8).is_err());
    }
}
