// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: MPL-2.0

use crate::ui::classes;
use crate::ui::components::CodeBlock;
use maud::{Markup, Render, html};

/// A display-only manual installation instruction.
pub struct InstallStep<'a> {
    pub label: &'a str,
    pub command: &'a str,
}

/// A reusable ordered list of display-only installation instructions.
pub struct InstallSteps<'a> {
    pub steps: Vec<InstallStep<'a>>,
}

impl Render for InstallSteps<'_> {
    fn render(&self) -> Markup {
        html! {
            ol class=(classes::install_steps::STEPS) {
                @for step in &self.steps {
                    li { (step.label) br; (CodeBlock { text: step.command }) ";" }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_instruction_content() {
        let rendered = InstallSteps {
            steps: vec![InstallStep {
                label: "<label>",
                command: "echo <command>",
            }],
        }
        .render()
        .into_string();
        assert!(rendered.contains("&lt;label&gt;"));
        assert!(rendered.contains("echo &lt;command&gt;"));
    }
}
