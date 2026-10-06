// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: MPL-2.0

use crate::ui::classes;
use maud::{Markup, Render, html};

pub struct Disclosure {
    pub summary: Markup,
    pub body: Markup,
}

impl Render for Disclosure {
    fn render(&self) -> Markup {
        html! {
            details class=(classes::disclosure::DISCLOSURE) {
                summary {
                    (self.summary.clone())
                }

                (self.body.clone())
            }
        }
    }
}
