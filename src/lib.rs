use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    memory::{sol_memcpy, sol_memset},
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::rent::Rent,
};
use pinocchio_log::log;
use pinocchio_system::instructions::Transfer;

pub const EXT_META_LEN: usize = 4;

#[repr(u8)]
pub enum StateExtensionError {
    ExtensionDataAleadyZerod,
    ExtensionDataIsNotInitialized,
}

impl From<StateExtensionError> for ProgramError {
    fn from(e: StateExtensionError) -> Self {
        Self::Custom(e as u32)
    }
}

pub trait ExtensionEnum: Sized + Clone + PartialEq + Eq {
    fn from_u8(ext_type: u8) -> Option<Self>;
    fn as_u8(&self) -> u8;
}

#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ExtensionState {
    Zerod = 0,
    Initialized = 1,
}

impl ExtensionEnum for ExtensionState {
    fn as_u8(&self) -> u8 {
        match self {
            ExtensionState::Initialized => 0,
            ExtensionState::Zerod => 1,
        }
    }

    fn from_u8(ext_type: u8) -> Option<Self> {
        match ext_type {
            0 => Some(Self::Initialized),
            1 => Some(Self::Zerod),
            _ => None,
        }
    }
}

pub trait Extension: Sized {
    const LEN: u16;

    type ExtensionEnum: ExtensionEnum;
    // enum used to identity Extension
    fn ext_type() -> u8;
    fn ext_len() -> u16 {
        Self::LEN
    }

    fn ext_with_meta_len() -> usize {
        Self::LEN as usize + EXT_META_LEN
    }

    unsafe fn pack(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self as *const Self as *const u8, Self::LEN as usize) }
    }

    unsafe fn unpack(bytes: &[u8]) -> Result<&Self, ProgramError> {
        if bytes.len() != Self::LEN as usize {
            return Err(ProgramError::InvalidAccountData);
        }

        unsafe { Ok(&*(bytes.as_ptr() as *const Self)) }
    }
}

#[derive(Debug)]
pub struct ExtensionInfo<'e, E: Extension> {
    pub ext: &'e E,
    pub position: usize,
    pub state: ExtensionState,
}

pub trait StateExtension {
    const BASE_STATE_LEN: usize;
    const OWNER_PROGRAM: Pubkey;
    const MAX_EXTENSIONS: u8;
    const EXT_START_MARKER: [u8; 8];

    fn len() -> usize {
        Self::BASE_STATE_LEN
    }

    fn check_ext_marker(bytes: &[u8]) -> bool {
        bytes == Self::EXT_START_MARKER.as_slice()
    }

    unsafe fn add_extension<E: Extension>(
        acc: &AccountInfo,
        fee_payer: &AccountInfo,
        rent: &AccountInfo,
        extension: &E,
    ) -> ProgramResult {
        log!("Add Extension : {}", E::ext_type());

        if unsafe { acc.owner() } != &Self::OWNER_PROGRAM {
            return Err(ProgramError::IllegalOwner);
        }

        if acc.data_is_empty() {
            return Err(ProgramError::InvalidAccountData);
        }

        let data_len = {
            let data = acc.try_borrow_data()?;

            if data.len() < Self::len() {
                return Err(ProgramError::InvalidAccountData);
            }

            data.len()
        };

        let rent = Rent::from_account_info(rent)?;

        let no_extensions = data_len == Self::len();

        // if appending for fist time
        let new_space_to_allocate = if no_extensions {
            Self::EXT_START_MARKER.len() + E::ext_with_meta_len()
        } else {
            E::ext_with_meta_len()
        };

        // transfer lamports for min rent exempt
        Transfer {
            from: fee_payer,
            to: acc,
            lamports: rent.minimum_balance(new_space_to_allocate),
        }
        .invoke()?;

        // realloc acc data and fill it with 0's
        acc.realloc(acc.data_len() + new_space_to_allocate, false)?;

        let mut data = acc.try_borrow_mut_data()?;

        let mut buffer = Vec::new();

        if no_extensions {
            buffer.extend_from_slice(Self::EXT_START_MARKER.as_slice());
        }

        unsafe {
            buffer.push(E::ext_type());
            buffer.push(ExtensionState::Initialized.as_u8());
            buffer.extend_from_slice(E::ext_len().to_le_bytes().as_slice());

            buffer.extend_from_slice(extension.pack());

            if let Some(data) = data.get_mut(data_len..) {
                sol_memcpy(data, &buffer, buffer.len());
            } else {
                return Err(ProgramError::InvalidAccountData);
            }
        };

        Ok(())
    }

    unsafe fn zero_out_extension_data<E: Extension>(
        acc: &AccountInfo,
        ext_type: E::ExtensionEnum,
    ) -> ProgramResult {
        log!("ZeroOut Extension : {}", E::ext_type());
        if let Some(ExtensionInfo {
            ext: _,
            position,
            state,
        }) = unsafe { Self::get_extension::<E>(acc, ext_type) }
        {
            let ext_data_start = position + EXT_META_LEN;
            if state == ExtensionState::Zerod {
                unsafe {
                    let mut data = acc.try_borrow_mut_data()?;

                    if let Some(data) = data.get_mut(ext_data_start..) {
                        sol_memset(data, 0, E::ext_len() as usize);
                    } else {
                        return Err(ProgramError::InvalidAccountData);
                    }
                }
            } else {
                return Err(StateExtensionError::ExtensionDataAleadyZerod.into());
            }
        }
        Ok(())
    }

    unsafe fn update_extension<E: Extension>(
        acc: &AccountInfo,
        ext_type: E::ExtensionEnum,
        extension: &E,
    ) -> ProgramResult {
        log!("Mutate Extension : {}", E::ext_type());

        if let Some(ExtensionInfo {
            ext: _,
            position,
            state,
        }) = unsafe { Self::get_extension::<E>(acc, ext_type) }
        {
            if state != ExtensionState::Zerod {
                unsafe {
                    let mut data = acc.try_borrow_mut_data()?;

                    let mut buffer = Vec::new();
                    buffer.push(E::ext_type());
                    buffer.push(ExtensionState::Initialized as u8);
                    buffer.extend_from_slice(E::ext_len().to_le_bytes().as_slice());
                    buffer.extend_from_slice(extension.pack());

                    if let Some(data) = data.get_mut(position..) {
                        sol_memcpy(data, &buffer, buffer.len());
                    }
                }
            } else {
                return Err(StateExtensionError::ExtensionDataIsNotInitialized.into());
            }
        }

        Ok(())
    }

    fn get_extension_variants<V: ExtensionEnum>(acc: &AccountInfo) -> Option<Vec<V>> {
        if unsafe { acc.owner() } != &Self::OWNER_PROGRAM {
            return None;
        }

        let data_len = acc.data_len();

        if data_len <= Self::len() {
            return None;
        }

        let data =
            unsafe { core::slice::from_raw_parts(acc.try_borrow_data().ok()?.as_ptr(), data_len) };

        Self::get_extension_variants_from_acc_data_uncheked(data)
    }

    fn get_extension_variants_from_acc_data_uncheked<V: ExtensionEnum>(
        data: &[u8],
    ) -> Option<Vec<V>> {
        let data_len = data.len();

        let ext_marker_start = Self::len();

        if !Self::check_ext_marker(
            data.get(ext_marker_start..(ext_marker_start + Self::EXT_START_MARKER.len()))?,
        ) {
            return None;
        }

        let mut ext_data_cursor = Self::len() + Self::EXT_START_MARKER.len();

        let mut extensions = Vec::new();

        while ext_data_cursor < data_len {
            let ext_type = match data.get(ext_data_cursor) {
                Some(ext_type) => *ext_type,
                None => break,
            };

            if let Some(ext) = V::from_u8(ext_type) {
                extensions.push(ext);
            }

            ext_data_cursor += 1;

            let _ext_state = data[ext_data_cursor];

            ext_data_cursor += 1;

            let ext_len: Option<u16> = data
                .get(ext_data_cursor..(ext_data_cursor + 2))
                .map(|d| d.try_into().ok().map(|d| u16::from_le_bytes(d)))
                .flatten();

            match ext_len {
                Some(ext_len) => {
                    ext_data_cursor += 2;
                    ext_data_cursor += ext_len as usize;
                }
                None => break,
            }
        }

        Some(extensions)
    }

    unsafe fn get_extension<'e, E: Extension>(
        acc: &AccountInfo,
        ext_type: E::ExtensionEnum,
    ) -> Option<ExtensionInfo<'e, E>> {
        if unsafe { acc.owner() } != &Self::OWNER_PROGRAM {
            return None;
        }

        let data_len = acc.data_len();

        if data_len < Self::len() + Self::EXT_START_MARKER.len() {
            return None;
        }

        let data =
            unsafe { core::slice::from_raw_parts(acc.try_borrow_data().ok()?.as_ptr(), data_len) };

        Self::get_extension_from_acc_data_unchecked(data, ext_type)
    }

    fn get_extension_from_acc_data_unchecked<'e, E: Extension>(
        data: &'e [u8],
        ext_type: E::ExtensionEnum,
    ) -> Option<ExtensionInfo<'e, E>> {
        let data_len = data.len();

        let ext_marker_start = Self::len();

        if !Self::check_ext_marker(
            data.get(ext_marker_start..ext_marker_start + Self::EXT_START_MARKER.len())?,
        ) {
            return None;
        }

        let mut ext_data_cursor = Self::len() + Self::EXT_START_MARKER.len();

        while ext_data_cursor < data_len {
            let ext_position = ext_data_cursor;
            let read_ext_type = data[ext_data_cursor];
            ext_data_cursor += 1;

            let ext_state = ExtensionState::from_u8(data[ext_data_cursor])?;

            ext_data_cursor += 1;

            let ext_len: Option<u16> = data
                .get(ext_data_cursor..(ext_data_cursor + 2))
                .map(|d| d.try_into().ok().map(|d| u16::from_le_bytes(d)))
                .flatten();

            match ext_len {
                Some(ext_len) => {
                    ext_data_cursor += 2;

                    let ext = unsafe {
                        E::unpack(&data[ext_data_cursor..(ext_data_cursor + ext_len as usize)]).ok()
                    };

                    ext_data_cursor += ext_len as usize;

                    if let Some(ext) = ext {
                        if read_ext_type == ext_type.as_u8() {
                            return Some(ExtensionInfo {
                                ext,
                                position: ext_position,
                                state: ext_state,
                            });
                        }
                    }
                }
                None => break,
            }
        }

        None
    }
}
