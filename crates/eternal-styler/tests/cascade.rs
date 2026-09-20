// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! End-to-end: a small widget tree, stylesheets, computed styles.

use eternal_styler::{
    BorderStyle, Color, ComputedStyle, DeclarationBlock, Element, ElementState, ErrorKind,
    FontWeight, LengthPercentage, LengthPercentageOrAuto, LengthPercentageOrNone, LineHeight,
    OpaqueElement, SelectorList, Styler, Stylesheet, TextAlign,
};

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

    fn node(&mut self, index: usize) -> &mut Node {
        &mut self.nodes[index]
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

fn styler(css: &str) -> Styler {
    let mut styler = Styler::new();
    styler.add_stylesheet(Stylesheet::parse(css).expect("valid stylesheet"));
    styler
}

fn hex(rgba: u32) -> Color {
    Color::from_packed_rgba(rgba)
}

fn px(v: f32) -> LengthPercentage {
    LengthPercentage::Px(v)
}

#[test]
fn specificity_source_order_and_importance() {
    let mut tree = Tree::default();
    let plain = tree.add(None, "button");
    let primary = tree.add(None, "button");
    tree.node(primary).classes.push("primary");
    let save = tree.add(None, "button");
    tree.node(save).classes.push("primary");
    tree.node(save).id = Some("save");

    let mut styler = styler(
        "button { color: #111111; padding: 4px; }
         .primary { color: #222222; }
         button { color: #333333; }
         #save { color: #444444; }",
    );

    let style = |styler: &Styler, index: usize| styler.compute(&tree.handle(index), None);
    assert_eq!(style(&styler, plain).color, hex(0x333333ff));
    assert_eq!(style(&styler, primary).color, hex(0x222222ff));
    assert_eq!(style(&styler, save).color, hex(0x444444ff));
    assert_eq!(style(&styler, save).padding.top, px(4.0));

    // A later stylesheet wins ties, and only ties.
    styler.add_stylesheet(Stylesheet::parse("button { color: #555555 }").unwrap());
    assert_eq!(style(&styler, plain).color, hex(0x555555ff));
    assert_eq!(style(&styler, primary).color, hex(0x222222ff));

    // !important beats everything that is not.
    styler.add_stylesheet(Stylesheet::parse("button { color: #666666 !important }").unwrap());
    assert_eq!(style(&styler, plain).color, hex(0x666666ff));
    assert_eq!(style(&styler, save).color, hex(0x666666ff));

    let rules = styler.matching_rules(&tree.handle(save));
    let selectors: Vec<String> = rules.iter().map(|r| r.selectors.to_string()).collect();
    assert_eq!(
        selectors,
        ["button", "button", "button", "button", ".primary", "#save"]
    );
}

#[test]
fn inheritance_and_relative_units() {
    let mut tree = Tree::default();
    let panel = tree.add(None, "panel");
    let label = tree.add(Some(panel), "label");
    let deep = tree.add(Some(label), "label");
    tree.node(deep).classes.push("deep");

    let mut styler = styler(
        "panel { font-size: 20px; color: #aabbcc; line-height: 1.5; letter-spacing: 0.1em;
                 background-color: #ffffff; text-align: center; font-weight: bold; }
         label { font-size: 1.5em; margin-top: 1em; padding-left: 50%; width: 2rem;
                 border: 1px solid; font-weight: bolder; }
         .deep { font-size: 120%; line-height: 2em; }",
    );
    styler.set_root_font_size(10.0);

    let panel_style = styler.compute(&tree.handle(panel), None);
    assert_eq!(panel_style.font_size, 20.0);
    assert_eq!(panel_style.letter_spacing, 2.0);
    assert_eq!(panel_style.font_weight, FontWeight::BOLD);

    let label_style = styler.compute(&tree.handle(label), Some(&panel_style));
    assert_eq!(label_style.font_size, 30.0); // 1.5em of the parent's 20px
    assert_eq!(label_style.margin.top, LengthPercentageOrAuto::Px(30.0)); // 1em of its own
    assert_eq!(label_style.padding.left, LengthPercentage::Percent(0.5));
    assert_eq!(label_style.width, LengthPercentageOrAuto::Px(20.0)); // 2rem of 10px
    assert_eq!(label_style.color, hex(0xaabbccff)); // inherited
    assert_eq!(label_style.line_height, LineHeight::Number(1.5)); // inherited as a number
    assert_eq!(label_style.letter_spacing, 2.0); // inherited as computed pixels
    assert_eq!(label_style.text_align, TextAlign::Center);
    assert_eq!(label_style.font_weight, FontWeight::new(900.0)); // bolder than bold
    assert_eq!(label_style.background_color, Color::TRANSPARENT); // not inherited
    assert_eq!(label_style.border_color.top, hex(0xaabbccff)); // currentcolor
    assert_eq!(label_style.border_width.top, 1.0);

    let deep_style = styler.compute(&tree.handle(deep), Some(&label_style));
    assert_eq!(deep_style.font_size, 36.0); // 120% of 30px
    assert_eq!(deep_style.line_height, LineHeight::Px(72.0)); // 2em of 36px
    assert_eq!(deep_style.line_height.resolve(36.0), Some(72.0));
    assert_eq!(deep_style.font_weight, FontWeight::new(900.0));
}

#[test]
fn interactive_and_custom_states() {
    let mut tree = Tree::default();
    let idle = tree.add(None, "button");
    let hovered = tree.add(None, "button");
    tree.node(hovered).state = ElementState::HOVER;
    let pressed = tree.add(None, "button");
    tree.node(pressed).state = ElementState::HOVER | ElementState::ACTIVE;
    let disabled = tree.add(None, "button");
    tree.node(disabled).state = ElementState::HOVER | ElementState::DISABLED;
    let loading = tree.add(None, "button");
    tree.node(loading).custom_states.push("loading");

    let styler = styler(
        "button { background-color: #000000; }
         button:hover { background-color: #111111; }
         button:active { background-color: #222222; }
         button:disabled { opacity: 0.5; background-color: #333333; }
         button:not(:disabled):hover { border-radius: 4px; }
         button:enabled:state(loading) { opacity: 0.8; }",
    );
    let style = |index: usize| styler.compute(&tree.handle(index), None);

    assert_eq!(style(idle).background_color, hex(0x000000ff));
    assert_eq!(style(hovered).background_color, hex(0x111111ff));
    assert_eq!(style(hovered).border_radius.top_left, px(4.0));
    assert_eq!(style(pressed).background_color, hex(0x222222ff));
    assert_eq!(style(disabled).background_color, hex(0x333333ff));
    assert_eq!(style(disabled).opacity, 0.5);
    assert_eq!(style(disabled).border_radius.top_left, px(0.0));
    assert_eq!(style(loading).opacity, 0.8);
    assert_eq!(style(idle).opacity, 1.0);
}

#[test]
fn tree_structural_selectors_and_attributes() {
    let mut tree = Tree::default();
    let list = tree.add(None, "list");
    tree.node(list).attrs.push(("role", "listbox"));
    let items: Vec<usize> = (0..4).map(|_| tree.add(Some(list), "item")).collect();
    tree.node(items[2]).classes.push("selected");
    tree.node(items[3]).attrs.push(("data-kind", "Header"));
    let footer = tree.add(Some(list), "footer");
    let _footer_text = tree.add(Some(footer), "text");

    let styler = styler(
        "item:first-child { margin-top: 1px; }
         item:nth-child(2n) { margin-right: 2px; }
         item:last-of-type { margin-bottom: 3px; }
         list > item { margin-left: 4px; }
         item.selected + item { padding-top: 5px; }
         item ~ footer { padding-right: 6px; }
         :root { padding-bottom: 7px; }
         item:empty { padding-left: 8px; }
         footer:not(:empty) { row-gap: 9px; }
         [role=listbox] { column-gap: 10px; }
         [data-kind^='head' i] { width: 11px; }
         list:has(> item.selected) { height: 12px; }
         :is(footer, text) { min-width: 13px; }
         item:nth-child(2 of .selected) { max-width: 1px; }
         item:only-child { max-width: 2px; }",
    );
    let style = |index: usize| styler.compute(&tree.handle(index), None);
    let auto = LengthPercentageOrAuto::Auto;
    let zero = LengthPercentageOrAuto::Px(0.0);

    assert_eq!(style(items[0]).margin.top, LengthPercentageOrAuto::Px(1.0));
    assert_eq!(style(items[1]).margin.top, zero);
    assert_eq!(
        style(items[1]).margin.right,
        LengthPercentageOrAuto::Px(2.0)
    );
    assert_eq!(style(items[2]).margin.right, zero);
    assert_eq!(
        style(items[3]).margin.bottom,
        LengthPercentageOrAuto::Px(3.0)
    );
    assert_eq!(style(items[2]).margin.bottom, zero);
    assert_eq!(style(items[0]).margin.left, LengthPercentageOrAuto::Px(4.0));
    assert_eq!(style(items[3]).padding.top, px(5.0));
    assert_eq!(style(items[2]).padding.top, px(0.0));
    assert_eq!(style(footer).padding.right, px(6.0));
    assert_eq!(style(list).padding.bottom, px(7.0));
    assert_eq!(style(items[0]).padding.bottom, px(0.0));
    assert_eq!(style(items[0]).padding.left, px(8.0));
    assert_eq!(style(footer).padding.left, px(0.0));
    assert_eq!(style(footer).row_gap, px(9.0));
    assert_eq!(style(list).column_gap, px(10.0));
    assert_eq!(style(items[3]).width, LengthPercentageOrAuto::Px(11.0));
    assert_eq!(style(items[2]).width, auto);
    assert_eq!(style(list).height, LengthPercentageOrAuto::Px(12.0));
    assert_eq!(style(footer).min_width, LengthPercentageOrAuto::Px(13.0));
    assert_eq!(style(items[0]).min_width, auto);
    assert_eq!(style(items[2]).max_width, LengthPercentageOrNone::None);
    assert_eq!(style(items[0]).max_width, LengthPercentageOrNone::None);

    // Selectors can also be matched on their own.
    let selected = SelectorList::parse("list item.selected:nth-child(3)").unwrap();
    assert!(selected.matches(&tree.handle(items[2])));
    assert!(!selected.matches(&tree.handle(items[1])));
}

#[test]
fn shorthands_and_borders() {
    let mut tree = Tree::default();
    let a = tree.add(None, "box");
    tree.node(a).classes.push("a");
    let b = tree.add(None, "box");
    tree.node(b).classes.push("b");
    let c = tree.add(None, "box");
    tree.node(c).classes.push("c");

    let styler = styler(
        ".a { margin: 1px 2px 3px 4px; padding: 5px 6px; gap: 7px 8px;
              border: 2px dashed #ff0000; border-radius: 1px 2px 3px; color: #00ff00; }
         .b { border-width: 4px; border-top: solid; border-left-style: hidden;
              border-color: #0000ff; }
         .c { border: thick solid; border-bottom-width: 0; margin: auto; }",
    );
    let style = |index: usize| styler.compute(&tree.handle(index), None);

    let a = style(a);
    assert_eq!(a.margin.top, LengthPercentageOrAuto::Px(1.0));
    assert_eq!(a.margin.right, LengthPercentageOrAuto::Px(2.0));
    assert_eq!(a.margin.bottom, LengthPercentageOrAuto::Px(3.0));
    assert_eq!(a.margin.left, LengthPercentageOrAuto::Px(4.0));
    assert_eq!(a.padding.top, px(5.0));
    assert_eq!(a.padding.right, px(6.0));
    assert_eq!(a.padding.bottom, px(5.0));
    assert_eq!(a.padding.left, px(6.0));
    assert_eq!(a.row_gap, px(7.0));
    assert_eq!(a.column_gap, px(8.0));
    assert_eq!(a.border_width.left, 2.0);
    assert_eq!(a.border_style.left, BorderStyle::Dashed);
    assert_eq!(a.border_color.left, hex(0xff0000ff));
    assert_eq!(a.border_radius.top_left, px(1.0));
    assert_eq!(a.border_radius.top_right, px(2.0));
    assert_eq!(a.border_radius.bottom_right, px(3.0));
    assert_eq!(a.border_radius.bottom_left, px(2.0));

    // Widths only count where there is a visible style.
    let b = style(b);
    assert_eq!(b.border_width.top, 3.0); // border-top reset the width to medium
    assert_eq!(b.border_style.top, BorderStyle::Solid);
    assert_eq!(b.border_color.top, hex(0x0000ffff));
    assert_eq!(b.border_width.right, 0.0); // 4px declared, but no style
    assert_eq!(b.border_width.left, 0.0); // hidden
    assert_eq!(b.border_style.left, BorderStyle::Hidden);

    let c = style(c);
    assert_eq!(c.border_width.top, 5.0);
    assert_eq!(c.border_width.bottom, 0.0);
    assert_eq!(c.border_color.top, Color::BLACK); // currentcolor
    assert_eq!(c.margin.left, LengthPercentageOrAuto::Auto);
}

#[test]
fn inline_styles_sit_between_normal_and_important() {
    let mut tree = Tree::default();
    let node = tree.add(None, "box");
    tree.node(node).id = Some("x");

    let styler = styler(
        "#x { color: #000000; width: 20px !important; height: 30px !important; opacity: 0.5 }",
    );
    let inline = DeclarationBlock::parse(
        "color: #ff00ff; width: 10px !important; height: 40px; opacity: 0.25",
    )
    .unwrap();
    let style = styler.compute_with_inline(&tree.handle(node), None, Some(&inline));

    assert_eq!(style.color, hex(0xff00ffff)); // inline normal beats sheet normal
    assert_eq!(style.width, LengthPercentageOrAuto::Px(10.0)); // inline important beats sheet important
    assert_eq!(style.height, LengthPercentageOrAuto::Px(30.0)); // sheet important beats inline normal
    assert_eq!(style.opacity, 0.25);
}

#[test]
fn css_wide_keywords() {
    let mut tree = Tree::default();
    let panel = tree.add(None, "panel");
    let child = tree.add(Some(panel), "child");

    let styler = styler(
        "panel { color: #123456; padding: 3px; margin: 2px; width: 50px; border: 1px solid #ff0000;
                 font-size: 40px; }
         child { color: initial; padding: inherit; margin: unset; width: unset;
                 border-color: initial; border-width: initial; border-style: inherit;
                 font-size: revert; }",
    );
    let parent = styler.compute(&tree.handle(panel), None);
    let style = styler.compute(&tree.handle(child), Some(&parent));

    assert_eq!(style.color, Color::BLACK); // initial, despite inheriting by default
    assert_eq!(style.padding.top, px(3.0)); // inherit, despite not inheriting by default
    assert_eq!(style.margin.top, LengthPercentageOrAuto::Px(0.0)); // unset: initial
    assert_eq!(style.width, LengthPercentageOrAuto::Auto);
    assert_eq!(style.border_style.top, BorderStyle::Solid); // inherited
    assert_eq!(style.border_width.top, 3.0); // initial is medium
    assert_eq!(style.border_color.top, Color::BLACK); // initial is currentcolor, which is now black
    assert_eq!(style.font_size, 40.0); // revert behaves as unset: inherit

    // The root's "parent" is the initial style.
    let root = styler.compute(&tree.handle(child), None);
    assert_eq!(root.padding.top, px(0.0));
    assert_eq!(root.font_size, 16.0);
    assert_eq!(ComputedStyle::default(), ComputedStyle::initial());
}

#[test]
fn errors_name_the_line() {
    let error = Stylesheet::parse("box {\n  width: 10px;\n  colour: red;\n}").unwrap_err();
    assert_eq!(
        error.kind(),
        &ErrorKind::UnknownProperty("colour".to_owned())
    );
    assert_eq!(error.line(), 3);
    assert_eq!(error.column(), 3);

    let (sheet, errors) = Stylesheet::parse_lenient("box { width: 10px; colour: red }");
    assert_eq!(errors.len(), 1);
    assert_eq!(sheet.rules()[0].declarations.len(), 1);
}
