// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A style inspector: styles a small fixed widget tree with a stylesheet and
//! prints, for every element, which rules matched and what it computed to.
//!
//! Pass a `.css` file to use your own stylesheet; without one, a built-in
//! example stylesheet is used. Run with
//! `bazel run //crates/eternal-styler:inspect_example -- theme.css` or
//! `cargo run --example inspect -- theme.css`.

use std::process::ExitCode;

use eternal_styler::{
    ComputedStyle, Element, ElementState, LengthPercentage, LengthPercentageOrAuto, OpaqueElement,
    Styler, Stylesheet,
};

const DEFAULT_CSS: &str = "
    window { font-family: monospace; font-size: 11px; color: #d8d8d8; }
    .toolbar { padding: 2px; gap: 2px; background-color: #333; }
    button { padding: 2px 6px; border: 1px solid #1c1c1c; background-color: #3f3f3f; }
    button:hover { background-color: #4b4b4b; }
    button:active { background-color: #262626; }
    button:disabled { color: #7a7a7a; }
    button.primary { background-color: #3b6ea5; color: white; }
    button:first-child { border-top-left-radius: 3px; }
    panel { padding: 0.5em; border: 1px solid #1c1c1c; }
    panel > label { font-size: 1.2em; font-weight: bold; }
    label[role=hint] { color: #9a9a9a; font-style: italic; }
    #status:state(busy) { opacity: 0.5; }
";

/// A node in a tiny arena-backed tree.
#[derive(Debug, Default)]
struct Node {
    name: &'static str,
    id: Option<&'static str>,
    classes: Vec<&'static str>,
    attrs: Vec<(&'static str, &'static str)>,
    state: ElementState,
    custom_states: Vec<&'static str>,
    parent: Option<usize>,
    children: Vec<usize>,
}

#[derive(Debug, Default)]
struct Tree {
    nodes: Vec<Node>,
}

impl Tree {
    fn add(&mut self, parent: Option<usize>, name: &'static str) -> usize {
        let index = self.nodes.len();
        self.nodes.push(Node {
            name,
            parent,
            ..Node::default()
        });
        if let Some(parent) = parent {
            self.nodes[parent].children.push(index);
        }
        index
    }

    fn handle(&self, index: usize) -> Handle<'_> {
        Handle { tree: self, index }
    }
}

#[derive(Clone, Debug)]
struct Handle<'a> {
    tree: &'a Tree,
    index: usize,
}

impl<'a> Handle<'a> {
    fn node(&self) -> &'a Node {
        &self.tree.nodes[self.index]
    }

    fn sibling(&self, offset: isize) -> Option<Self> {
        let parent = self.node().parent?;
        let siblings = &self.tree.nodes[parent].children;
        let position = siblings.iter().position(|&i| i == self.index)?;
        let target = position.checked_add_signed(offset)?;
        siblings.get(target).map(|&i| self.tree.handle(i))
    }

    /// The element as a selector would be written: `button#save.primary`.
    fn describe(&self) -> String {
        let node = self.node();
        let mut text = node.name.to_owned();
        if let Some(id) = node.id {
            text.push('#');
            text.push_str(id);
        }
        for class in &node.classes {
            text.push('.');
            text.push_str(class);
        }
        text
    }
}

impl Element for Handle<'_> {
    fn opaque(&self) -> OpaqueElement {
        OpaqueElement::new(self.node())
    }

    fn parent(&self) -> Option<Self> {
        self.node().parent.map(|i| self.tree.handle(i))
    }

    fn prev_sibling(&self) -> Option<Self> {
        self.sibling(-1)
    }

    fn next_sibling(&self) -> Option<Self> {
        self.sibling(1)
    }

    fn first_child(&self) -> Option<Self> {
        self.node().children.first().map(|&i| self.tree.handle(i))
    }

    fn local_name(&self) -> &str {
        self.node().name
    }

    fn id(&self) -> Option<&str> {
        self.node().id
    }

    fn classes(&self) -> impl Iterator<Item = &str> {
        self.node().classes.iter().copied()
    }

    fn attribute(&self, name: &str) -> Option<&str> {
        self.node()
            .attrs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| *value)
    }

    fn state(&self) -> ElementState {
        self.node().state
    }

    fn has_custom_state(&self, name: &str) -> bool {
        self.node().custom_states.contains(&name)
    }
}

fn demo_tree() -> Tree {
    let mut tree = Tree::default();
    let window = tree.add(None, "window");
    let toolbar = tree.add(Some(window), "row");
    tree.nodes[toolbar].classes.push("toolbar");
    let save = tree.add(Some(toolbar), "button");
    tree.nodes[save].id = Some("save");
    tree.nodes[save].classes.push("primary");
    tree.nodes[save].state = ElementState::HOVER;
    let undo = tree.add(Some(toolbar), "button");
    tree.nodes[undo].id = Some("undo");
    tree.nodes[undo].state = ElementState::HOVER | ElementState::ACTIVE;
    let redo = tree.add(Some(toolbar), "button");
    tree.nodes[redo].id = Some("redo");
    tree.nodes[redo].state = ElementState::DISABLED;
    let panel = tree.add(Some(window), "panel");
    let title = tree.add(Some(panel), "label");
    tree.nodes[title].id = Some("title");
    let hint = tree.add(Some(panel), "label");
    tree.nodes[hint].attrs.push(("role", "hint"));
    let status = tree.add(Some(panel), "label");
    tree.nodes[status].id = Some("status");
    tree.nodes[status].custom_states.push("busy");
    tree
}

fn main() -> ExitCode {
    let css = match std::env::args().nth(1) {
        Some(path) => match std::fs::read_to_string(&path) {
            Ok(css) => css,
            Err(error) => {
                eprintln!("{path}: {error}");
                return ExitCode::FAILURE;
            }
        },
        None => DEFAULT_CSS.to_owned(),
    };
    let (sheet, errors) = Stylesheet::parse_lenient(&css);
    for error in &errors {
        eprintln!("warning: {error}");
    }
    println!("{} rule(s) loaded\n", sheet.len());

    let mut styler = Styler::new();
    styler.add_stylesheet(sheet);

    let tree = demo_tree();
    // Walk top-down so each parent's computed style is ready for its
    // children, which is what a real toolkit does too.
    let mut styles: Vec<Option<ComputedStyle>> = vec![None; tree.nodes.len()];
    for index in 0..tree.nodes.len() {
        let handle = tree.handle(index);
        let parent = tree.nodes[index].parent.and_then(|p| styles[p].as_ref());
        let style = styler.compute(&handle, parent);
        print_element(&styler, &handle, &style);
        styles[index] = Some(style);
    }
    ExitCode::SUCCESS
}

fn print_element(styler: &Styler, handle: &Handle<'_>, style: &ComputedStyle) {
    let depth = std::iter::successors(handle.parent(), |h| h.parent()).count();
    let indent = "  ".repeat(depth);
    let state = handle.state();
    let flags: Vec<&str> = [
        (ElementState::HOVER, ":hover"),
        (ElementState::ACTIVE, ":active"),
        (ElementState::FOCUS, ":focus"),
        (ElementState::DISABLED, ":disabled"),
        (ElementState::CHECKED, ":checked"),
    ]
    .into_iter()
    .filter(|(flag, _)| state.contains(*flag))
    .map(|(_, name)| name)
    .collect();
    println!("{indent}{}{}", handle.describe(), flags.concat());

    let rules = styler.matching_rules(handle);
    if rules.is_empty() {
        println!("{indent}  (no rules match)");
    }
    for rule in rules {
        println!("{indent}  matches: {}", rule.selectors);
    }
    println!(
        "{indent}  color {}  background {}  opacity {}",
        hex(style.color),
        hex(style.background_color),
        style.opacity
    );
    println!(
        "{indent}  font {} {}px weight {} {:?}",
        style.font_family.names().join(", "),
        style.font_size,
        style.font_weight.value(),
        style.font_style
    );
    println!(
        "{indent}  padding {} {} {} {}  border {} {} {} {}  radius {}",
        lp(style.padding.top),
        lp(style.padding.right),
        lp(style.padding.bottom),
        lp(style.padding.left),
        style.border_width.top,
        style.border_width.right,
        style.border_width.bottom,
        style.border_width.left,
        lp(style.border_radius.top_left),
    );
    println!(
        "{indent}  width {}  height {}",
        auto(style.width),
        auto(style.height)
    );
}

fn hex(color: eternal_styler::Color) -> String {
    let [r, g, b, a] = color.to_rgba8();
    format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
}

fn lp(value: LengthPercentage) -> String {
    match value {
        LengthPercentage::Px(px) => format!("{px}px"),
        LengthPercentage::Percent(p) => format!("{}%", p * 100.0),
    }
}

fn auto(value: LengthPercentageOrAuto) -> String {
    match value {
        LengthPercentageOrAuto::Auto => "auto".to_owned(),
        LengthPercentageOrAuto::Px(px) => format!("{px}px"),
        LengthPercentageOrAuto::Percent(p) => format!("{}%", p * 100.0),
    }
}
