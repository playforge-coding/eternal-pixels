// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/// Builds an [`Element`](crate::Element) tree from markup.
///
/// The syntax is a small subset of HTML, inside Rust:
///
/// ```
/// use eternal_ui::{ui, Element};
///
/// #[derive(Clone, Debug)]
/// enum Msg { Save, Rename(String), Wrap(bool) }
///
/// let name = "sketch.png";
/// let root: Element<Msg> = ui! {
///     <column class="panel" id="main">
///         <row class="toolbar">
///             <button id="save" shortcut="Ctrl+S" on:click={|| Msg::Save}>"Save"</button>
///             <spacer/>
///             <label class="dim">{format!("Editing {name}")}</label>
///         </row>
///         <input value={name} placeholder="File name" on:change={Msg::Rename}/>
///         <checkbox checked="true" on:toggle={Msg::Wrap}>"Wrap around"</checkbox>
///     </column>
/// };
/// assert_eq!(root.tag(), "column");
/// assert_eq!(root.children().len(), 3);
/// ```
///
/// - A tag is one of the constructors in [`elements`](crate::elements):
///   `column`, `row`, `panel`, `label`, `button`, `checkbox`, `input`,
///   `separator`, `spacer`. Elements can be self-closing (`<spacer/>`).
/// - `name="literal"` and `name={expression}` set attributes, as
///   [`Element::attr`](crate::Element::attr) does. The expression can be
///   anything with a `Display` impl.
/// - `on:click={handler}` attaches a handler; the name is one of the
///   functions in [`on`](crate::on) and the handler is a closure with the
///   matching signature.
/// - A child is another tag, a string literal, or `{expression}` where the
///   expression is an element, a `Vec` of elements, an `Option<Element>`, or
///   a `String`. Text inside a label becomes its text; text elsewhere
///   becomes a child label.
///
/// A closing tag has to match its opening tag, or the macro produces a type
/// error naming both. Every token costs a level of macro recursion, so a
/// large template may need `#![recursion_limit = "512"]` on the crate.
#[macro_export]
macro_rules! ui {
    // Closing tag: pop the frame, checking that the tags match.
    (@m [ [ $tag:ident [ $($calls:tt)* ] ] $($stack:tt)* ] < / $close:ident > $($rest:tt)*) => {
        $crate::ui!(@close [ [ $tag [ $($calls)* ] ] $($stack)* ] $close $($rest)*)
    };
    // Opening tag: push a frame and parse its attributes.
    (@m [ $($stack:tt)* ] < $tag:ident $($rest:tt)*) => {
        $crate::ui!(@a [ [ $tag [ ] ] $($stack)* ] $($rest)*)
    };
    // A text child.
    (@m [ [ $tag:ident [ $($calls:tt)* ] ] $($stack:tt)* ] $text:literal $($rest:tt)*) => {
        $crate::ui!(@m [ [ $tag [ $($calls)* .child($text) ] ] $($stack)* ] $($rest)*)
    };
    // An expression child.
    (@m [ [ $tag:ident [ $($calls:tt)* ] ] $($stack:tt)* ] { $child:expr } $($rest:tt)*) => {
        $crate::ui!(@m [ [ $tag [ $($calls)* .child($child) ] ] $($stack)* ] $($rest)*)
    };

    // A handler attribute.
    (@a [ [ $tag:ident [ $($calls:tt)* ] ] $($stack:tt)* ] on : $event:ident = { $handler:expr } $($rest:tt)*) => {
        $crate::ui!(@a [ [ $tag [ $($calls)* .on($crate::on::$event($handler)) ] ] $($stack)* ] $($rest)*)
    };
    // A literal attribute.
    (@a [ [ $tag:ident [ $($calls:tt)* ] ] $($stack:tt)* ] $name:ident = $value:literal $($rest:tt)*) => {
        $crate::ui!(@a [ [ $tag [ $($calls)* .attr(stringify!($name), $value) ] ] $($stack)* ] $($rest)*)
    };
    // An expression attribute.
    (@a [ [ $tag:ident [ $($calls:tt)* ] ] $($stack:tt)* ] $name:ident = { $value:expr } $($rest:tt)*) => {
        $crate::ui!(@a [ [ $tag [ $($calls)* .attr(stringify!($name), $value) ] ] $($stack)* ] $($rest)*)
    };
    // End of the opening tag: on to the children.
    (@a [ $($stack:tt)* ] > $($rest:tt)*) => {
        $crate::ui!(@m [ $($stack)* ] $($rest)*)
    };
    // A self-closing tag.
    (@a [ [ $tag:ident [ $($calls:tt)* ] ] $($stack:tt)* ] / > $($rest:tt)*) => {
        $crate::ui!(@close [ [ $tag [ $($calls)* ] ] $($stack)* ] $tag $($rest)*)
    };

    // The root closed and nothing follows: build it.
    (@close [ [ $tag:ident [ $($calls:tt)* ] ] ] $close:ident) => {{
        let _: $crate::tags::$tag = $crate::tags::$close;
        $crate::elements::$tag() $($calls)*
    }};
    // A child closed: build it and add it to its parent.
    (@close [ [ $tag:ident [ $($calls:tt)* ] ] [ $parent:ident [ $($parent_calls:tt)* ] ] $($stack:tt)* ] $close:ident $($rest:tt)*) => {
        $crate::ui!(@m [ [ $parent [ $($parent_calls)* .child({
            let _: $crate::tags::$tag = $crate::tags::$close;
            $crate::elements::$tag() $($calls)*
        }) ] ] $($stack)* ] $($rest)*)
    };

    // Entry point.
    (< $($rest:tt)*) => {
        $crate::ui!(@m [ ] < $($rest)*)
    };
}
