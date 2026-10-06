// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: MPL-2.0

use crate::ui::classes;
use maud::{Markup, Render, html};

pub struct CodeBlock<'a> {
    pub text: &'a str,
}

impl Render for CodeBlock<'_> {
    fn render(&self) -> Markup {
        html! {
            code class=(classes::code_block::CODE) { (self.text) }
        }
    }
}
