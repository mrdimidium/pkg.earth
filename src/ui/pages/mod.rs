// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: MPL-2.0

mod index;
mod licenses;

pub use index::{GoRelease, GoReleaseFile, IndexPage, ZigRelease, ZigReleaseFile};
pub use licenses::{LicenseEntry, LicenseOverview, LicenseUse, LicensesPage};
