// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: MPL-2.0

pub mod components;
pub mod pages;

pub(crate) mod classes {
    include!(concat!(env!("OUT_DIR"), "/css_modules.rs"));
}

mod generated_assets {
    include!(concat!(env!("OUT_DIR"), "/assets.rs"));
}

pub const APPLICATION: &[dimidiumlabs_ui::Asset] = generated_assets::APPLICATION;
