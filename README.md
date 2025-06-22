# 🧬 Solana State Extensions

**Solana State Extensions** is a lightweight, modular API designed to **extend and manipulate on-chain account state** on the Solana blockchain. This framework enables developers to dynamically **add**, **update**, **remove**, and **query extensions** on top of existing account state — without breaking compatibility or layout.

---

## ✨ Features

- 🔧 **Add Extensions**  
  Attach additional data or behaviors to existing accounts on-chain.

- 📝 **Update Extensions**  
  Modify the data within an existing extension atomically and safely.

- ❌ **Remove Extensions**  
  Cleanly delete an extension from an account’s state buffer.

- 📚 **List All Extension Variants**  
  Enumerate supported or currently present extensions.

- 📦 **Access Extension Data**  
  Retrieve deserialized extension payloads directly from on-chain accounts.

---

## 🧩 Use Cases

- Modular protocol upgrades without migrating account layout
- Feature toggles via extension flags
- Plugin-style architecture for DeFi, staking, governance, etc.
- Clean separation of core state and optional features

---

## 🚀 Getting Started

### Add the crate

```toml
# In Cargo.toml
solana-state-extensions = { git = "https://github.com/Nagaprasadvr/solana-state-extensions", branch = "main" }

```

## Snippets

# Api usage in on-chain program

```rust

// 🧩 Add multiple extensions
unsafe {
    BaseState::add_extension(
        my_state_acc,
        fee_payer,
        rent,
        &Ext1 {
            id: 255,
            data: [4; 32],
        },
    )?;
}

unsafe {
    BaseState::add_extension(
        my_state_acc,
        fee_payer,
        rent,
        &Ext2 {
            id: 10,
            check: true,
            owner: Pubkey::default(),
            data: [9; 32],
        },
    )?;
}

unsafe {
    BaseState::add_extension(
        my_state_acc,
        fee_payer,
        rent,
        &Ext3 {
            id: 50,
            payer: Pubkey::default(),
            authority: Pubkey::default(),
            data: [9; 32],
        },
    )?;
}

// 🔁 Update a specific extension
unsafe {
    BaseState::update_extension(
        my_state_acc,
        ExtEnum::Ext1,
        &Ext1 {
            id: 1,
            data: [7; 32],
        },
    )?;
}

// 🧽 Zero out extension data
unsafe {
    BaseState::zero_out_extension_data::<Ext1>(my_state_acc, ExtEnum::Ext1)?;
}

```

# Implementation

```rust

#[repr(C)] //keeps the struct layout the same across different architectures
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BaseState {
    pub is_initialized: u8,
    pub owner: Pubkey,
    pub state: State,
    pub data: [u8; 32],
    pub update_count: u32,
    pub bump: u8,
}

#[repr(C)]
#[derive(Debug)]
pub struct Ext1 {
    id: u8,
    data: [u8; 32],
}

#[repr(C)]
#[derive(Debug)]
pub struct Ext2 {
    id: u8,
    data: [u8; 32],
    owner: Pubkey,
    check: bool,
}

#[repr(C)]
#[derive(Debug)]
pub struct Ext3 {
    id: u8,
    data: [u8; 32],
    payer: Pubkey,
    authority: Pubkey,
}

impl StateExtension for BaseState {
    const OWNER_PROGRAM: Pubkey = crate::ID;

    const MAX_EXTENSIONS: u8 = 5;

    const EXT_START_MARKER: [u8; 8] = [167, 97, 34, 56, 78, 90, 102, 46];

    const BASE_STATE_LEN: usize = 76;
}

#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtEnum {
    Ext1,
    Ext2,
    Ext3,
}

impl ExtensionEnum for ExtEnum {
    fn from_u8(ext_type: u8) -> Option<Self> {
        match ext_type {
            0 => Some(ExtEnum::Ext1),
            1 => Some(ExtEnum::Ext2),
            2 => Some(ExtEnum::Ext3),
            _ => None,
        }
    }

    fn as_u8(&self) -> u8 {
        match self {
            ExtEnum::Ext1 => 0,
            ExtEnum::Ext2 => 1,
            ExtEnum::Ext3 => 2,
        }
    }
}

impl Extension for Ext1 {
    const LEN: u16 = 33;

    type ExtensionEnum = ExtEnum;

    fn ext_type() -> u8 {
        ExtEnum::Ext1 as u8
    }
}

impl Extension for Ext2 {
    const LEN: u16 = 66;

    type ExtensionEnum = ExtEnum;
    fn ext_type() -> u8 {
        ExtEnum::Ext2 as u8
    }
}

impl Extension for Ext3 {
    const LEN: u16 = 97;

    type ExtensionEnum = ExtEnum;

    fn ext_type() -> u8 {
        ExtEnum::Ext3 as u8
    }
}

```
