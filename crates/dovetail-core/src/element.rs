//! Elements: the boxes that layout places and painting draws.
//!
//! An element is plain data. `dovetail-layout` lays it out and
//! `dovetail-render` paints it, so this crate needs neither.

use std::any::Any;
use std::rc::Rc;

use crate::drag::DropHandler;
use crate::focus::FocusTarget;
use crate::input::{
  AccessAction, ImeEvent, KeyHandler, PointerHandler, WheelEvent,
};
use crate::view::{Callback, View};
use crate::window::CloseGuard;

/// A colour with red, green, blue and alpha parts, each from 0.0 to 1.0, in
/// the sRGB colour space (as in CSS and most design tools).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Color {
  /// Red, from 0.0 to 1.0.
  pub r: f32,
  /// Green, from 0.0 to 1.0.
  pub g: f32,
  /// Blue, from 0.0 to 1.0.
  pub b: f32,
  /// Alpha (opacity), from 0.0 (clear) to 1.0 (solid).
  pub a: f32,
}

impl Color {
  /// Fully clear.
  pub const TRANSPARENT: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
  /// Solid black.
  pub const BLACK: Color = Color::rgb(0.0, 0.0, 0.0);
  /// Solid white.
  pub const WHITE: Color = Color::rgb(1.0, 1.0, 1.0);

  /// A solid colour from red, green and blue parts.
  pub const fn rgb(
    r: f32,
    g: f32,
    b: f32,
  ) -> Self {
    Self { r, g, b, a: 1.0 }
  }

  /// A solid colour from a hex value such as `0x1e66f5`.
  pub const fn hex(value: u32) -> Self {
    Self::rgb(
      ((value >> 16) & 0xff) as f32 / 255.0,
      ((value >> 8) & 0xff) as f32 / 255.0,
      (value & 0xff) as f32 / 255.0,
    )
  }
}

/// How an element arranges its children.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ElementKind {
  /// Children side by side, left to right.
  Row,
  /// Children one below the other.
  Column,
  /// Children on top of each other, the last one in front.
  Stack,
  /// One box around its children, which are stacked. The default.
  #[default]
  Box,
}

/// Space on each side, in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges {
  /// Space above.
  pub top: f32,
  /// Space on the right.
  pub right: f32,
  /// Space below.
  pub bottom: f32,
  /// Space on the left.
  pub left: f32,
}

impl Edges {
  /// The same space on every side.
  pub const fn all(value: f32) -> Self {
    Self {
      top: value,
      right: value,
      bottom: value,
      left: value,
    }
  }

  /// `x` on the left and right, `y` above and below.
  pub const fn xy(
    x: f32,
    y: f32,
  ) -> Self {
    Self { top: y, right: x, bottom: y, left: x }
  }

  /// Left plus right.
  pub fn horizontal(self) -> f32 {
    self.left + self.right
  }

  /// Top plus bottom.
  pub fn vertical(self) -> f32 {
    self.top + self.bottom
  }
}

/// How big an element wants to be along one axis.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Size {
  /// Just big enough for its content. The default.
  #[default]
  Fit,
  /// Exactly this many logical pixels.
  Fixed(f32),
  /// All the space its parent offers on this axis.
  Fill,
}

/// Where children go across the axis of a row or column, or in a stack.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Align {
  /// At the start: the top in a row, the left in a column. The default.
  #[default]
  Start,
  /// In the middle.
  Center,
  /// At the end.
  End,
  /// As big as the space allows.
  Stretch,
}

/// Where children go along the axis of a row or column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Justify {
  /// Packed at the start. The default.
  #[default]
  Start,
  /// Packed in the middle.
  Center,
  /// Packed at the end.
  End,
  /// Spread out, with the first at the start and the last at the end.
  SpaceBetween,
}

/// A line around an element.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Border {
  /// Width in logical pixels, inside the element's edge.
  pub width: f32,
  /// Colour of the line.
  pub color: Color,
}

/// Where a layer goes (M08). A layer is drawn above everything else in the
/// window and is the first to receive the pointer, but keeps its place in
/// the tree for keys, focus and accessibility.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Layer {
  /// Over the whole window, as a backdrop behind a menu or a dialog.
  Fill,
  /// Below its parent element, lined up with its left edge, as a drop-down
  /// menu. It moves to fit inside the window.
  Below,
  /// Centred across the window, this many logical pixels from the top, as
  /// command search.
  TopCentre(f32),
  /// At this point in the window, in logical pixels.
  At(f32, f32),
}

/// How an element looks and is laid out. Every field has a plain default.
#[derive(Clone, Debug, PartialEq)]
pub struct Style {
  /// Space inside the element's edge, around its children.
  pub padding: Edges,
  /// Space between children of a row or column.
  pub gap: f32,
  /// Where children go across the main axis.
  pub align: Align,
  /// Where children go along the main axis.
  pub justify: Justify,
  /// How wide the element wants to be.
  pub width: Size,
  /// How tall the element wants to be.
  pub height: Size,
  /// The smallest width allowed.
  pub min_width: f32,
  /// The smallest height allowed.
  pub min_height: f32,
  /// The largest width allowed.
  pub max_width: f32,
  /// The largest height allowed.
  pub max_height: f32,
  /// A share of the space left over in a row or column. 0 takes none.
  pub grow: f32,
  /// Fill colour, painted first.
  pub background: Color,
  /// A line inside the element's edge.
  pub border: Border,
  /// Radius of the corners, in logical pixels.
  pub radius: f32,
  /// Children are not drawn outside the element's box.
  pub clip: bool,
  /// Shows the element as a layer above the window's content.
  pub layer: Option<Layer>,
  /// While this layer shows, Tab moves focus only inside it.
  pub modal: bool,
  /// The element scrolls: children bigger than it can be moved into view,
  /// with the mouse wheel, and to keep a shown caret (`text_cursor`)
  /// visible. Layout keeps the position. Use it with `clip`.
  pub scroll: bool,
  /// A scrolling element shows scrollbars while its content is larger
  /// than it is (M15). True by default; single-line text fields turn it
  /// off.
  pub scrollbars: bool,
  /// Size of text inside, in logical pixels. Children inherit it.
  pub font_size: Option<f32>,
  /// Colour of text inside. Children inherit it.
  pub text_color: Option<Color>,
  /// Weight of text inside (M14). Children inherit it.
  pub font_weight: Option<FontWeight>,
  /// Moves the element and everything in it this far from where layout
  /// puts it, in logical pixels, right and down, without moving anything
  /// else, as a CSS translate does (M18). Clicks, the focus ring and
  /// assistive tools follow it. The parent's clip still applies.
  pub offset: (f32, f32),
  /// How solid the element and everything in it are, from 0 (clear) to 1
  /// (solid, the default) (M18). It multiplies with the parents'.
  ///
  /// Each thing inside is drawn at this opacity on its own, so where
  /// children overlap they show through each other. Canvases are not
  /// faded. An element at 0 is still laid out and can still be clicked.
  pub opacity: f32,
}

/// How heavy text is drawn, from 100 (thin) to 900 (black), as in CSS and
/// OpenType (M14).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FontWeight(pub u16);

impl FontWeight {
  /// 300.
  pub const LIGHT: FontWeight = FontWeight(300);
  /// 400, the usual weight.
  pub const REGULAR: FontWeight = FontWeight(400);
  /// 500.
  pub const MEDIUM: FontWeight = FontWeight(500);
  /// 600.
  pub const SEMIBOLD: FontWeight = FontWeight(600);
  /// 700.
  pub const BOLD: FontWeight = FontWeight(700);
}

impl Default for FontWeight {
  fn default() -> Self {
    FontWeight::REGULAR
  }
}

impl Default for Style {
  fn default() -> Self {
    Self {
      padding: Edges::default(),
      gap: 0.0,
      align: Align::Start,
      justify: Justify::Start,
      width: Size::Fit,
      height: Size::Fit,
      min_width: 0.0,
      min_height: 0.0,
      max_width: f32::INFINITY,
      max_height: f32::INFINITY,
      grow: 0.0,
      background: Color::TRANSPARENT,
      border: Border::default(),
      radius: 0.0,
      clip: false,
      scroll: false,
      scrollbars: true,
      layer: None,
      modal: false,
      font_size: None,
      text_color: None,
      font_weight: None,
      offset: (0.0, 0.0),
      opacity: 1.0,
    }
  }
}

impl Style {
  /// True when a change from `self` to `other` can move or resize
  /// something, so layout must run again. Other changes need only painting.
  pub fn affects_layout(
    &self,
    other: &Style,
  ) -> bool {
    self.padding != other.padding
      || self.gap != other.gap
      || self.align != other.align
      || self.justify != other.justify
      || self.width != other.width
      || self.height != other.height
      || self.min_width != other.min_width
      || self.min_height != other.min_height
      || self.max_width != other.max_width
      || self.max_height != other.max_height
      || self.grow != other.grow
      || self.border.width != other.border.width
      || self.font_size != other.font_size
      || self.font_weight != other.font_weight
      || self.layer != other.layer
      // These move or fade boxes, which layout works out, reusing the
      // sizes it has.
      || self.offset != other.offset
      || self.opacity != other.opacity
  }
}

/// What an element is, for assistive tools such as screen readers (M07).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Role {
  /// Nothing in particular: a container, or decoration. The default.
  #[default]
  None,
  /// Presses when activated. Its name comes from the text inside.
  Button,
  /// An on or off switch (see [`Element::toggled`]).
  Switch,
  /// A check box (see [`Element::toggled`]).
  CheckBox,
  /// A box for one line of text.
  TextInput,
  /// A box for several lines of text.
  MultilineTextInput,
  /// A piece of text that says what is near it.
  Label,
  /// A heading.
  Heading,
  /// A group of related controls.
  Group,
  /// A picture. Give it a [`Element::label`].
  Image,
  /// A bar of menu titles.
  MenuBar,
  /// An open menu.
  Menu,
  /// An item in a menu, or a title in a menu bar. Its name comes from the
  /// text inside, unless it has a label.
  MenuItem,
  /// A list to choose from, such as command search's results.
  ListBox,
  /// One choice in a list.
  ListBoxOption,
  /// A row of tool buttons.
  Toolbar,
  /// A view the application draws itself, such as a drawing or a board.
  /// Give it a [`Element::label`] that says what it shows.
  Canvas,
  /// A row of tabs (M10).
  TabList,
  /// One tab. [`Element::toggled`] says whether it is the chosen one.
  Tab,
  /// What the chosen tab shows. Give it a [`Element::label`].
  TabPanel,
  /// A bar between two panels that is dragged to resize them.
  Splitter,
  /// A bar that shows how far work has got (M11). Its value is
  /// [`Element::value`].
  ProgressBar,
  /// A box that asks a question, above the rest of the window (M11). Give
  /// it a [`Element::label`].
  Dialog,
  /// One of a set of choices, of which one is chosen (M15).
  /// [`Element::toggled`] says whether it is the chosen one.
  RadioButton,
  /// A set of radio buttons. Give it a [`Element::label`].
  RadioGroup,
  /// A control that opens a list to choose from (M15). Its value is the
  /// chosen item's text, given as
  /// [`Element::text_value`].
  ComboBox,
  /// A number that can be typed, or stepped up and down (M15).
  SpinButton,
  /// A value in a range, set by dragging (M15). See [`Element::value_in`].
  Slider,
  /// Short help about another element (M15).
  Tooltip,
  /// A piece of text that goes somewhere when pressed (M15).
  Link,
  /// A bar along the bottom of a window that shows what is going on (M15).
  StatusBar,
  /// A line between groups of things (M15).
  Separator,
  /// Items that open and close to show the items inside them (M16).
  Tree,
  /// One item in a tree. See [`Element::item`] and [`Element::expanded`].
  TreeItem,
  /// Rows and columns, with headers (M16). See [`Element::table_size`].
  Table,
  /// One row of a table (M16).
  Row,
  /// One cell of a table row (M16).
  Cell,
  /// The header of a table column (M16). See [`Element::sorted`].
  ColumnHeader,
  /// A menu item that is on or off (M25). [`Element::toggled`] says which.
  MenuItemCheckBox,
  /// A menu item that is one of a group, of which one is on (M25).
  /// [`Element::toggled`] says whether it is the one.
  MenuItemRadio,
  /// A swatch that shows a colour, and opens a picker to change it (M25).
  /// Its value is the colour as text, given as [`Element::text_value`].
  ColourWell,
  /// A field for a date (M25). Its value is the date as text.
  DateInput,
  /// A field for a time of day (M25). Its value is the time as text.
  TimeInput,
  /// Rows and columns of cells moved through with the arrow keys, as a
  /// calendar's days (M25).
  Grid,
  /// One cell of a [`Role::Grid`] (M25). [`Element::selected`] says whether
  /// it is chosen.
  GridCell,
}

/// The range of a value, for sliders and number fields (M15).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ValueRange {
  /// The smallest value.
  pub min: f32,
  /// The largest value.
  pub max: f32,
  /// How far one step moves it, as an arrow key does. 0 means any value.
  pub step: f32,
}

/// What a canvas element draws with (M09): a value only the renderer
/// opens, so this crate needs no GPU types. Make one with
/// `dovetail_render::canvas`. Two are equal only if they are the same
/// value, so a new one repaints the canvas.
#[derive(Clone)]
pub struct CanvasSource(Rc<dyn Any>);

impl CanvasSource {
  /// Wraps `value` for the renderer.
  pub fn new<T: 'static>(value: T) -> CanvasSource {
    CanvasSource(Rc::new(value))
  }

  /// The value, if it is a `T`.
  pub fn get<T: 'static>(&self) -> Option<&T> {
    self.0.downcast_ref()
  }

  /// A number that is the same only for the same value.
  pub fn id(&self) -> usize {
    Rc::as_ptr(&self.0).cast::<()>() as usize
  }
}

impl PartialEq for CanvasSource {
  fn eq(
    &self,
    other: &Self,
  ) -> bool {
    Rc::ptr_eq(&self.0, &other.0)
  }
}

impl std::fmt::Debug for CanvasSource {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> std::fmt::Result {
    f.write_str("CanvasSource")
  }
}

/// The caret and selection of editable text inside an element (M06).
///
/// Offsets are bytes into the element's first text child, on grapheme
/// boundaries. The selection runs between `anchor` and `caret`; when they
/// are equal nothing is selected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextCursor {
  /// Where the caret is.
  pub caret: usize,
  /// The other end of the selection.
  pub anchor: usize,
  /// The caret is drawn. It is hidden while the element lacks focus.
  pub show_caret: bool,
  /// Text an input method is composing, as a byte range of the text. It
  /// is drawn underlined.
  pub composing: Option<(usize, usize)>,
}

/// A box that layout places and painting draws.
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
  /// How the children are arranged.
  pub kind: ElementKind,
  /// How it looks and is laid out.
  pub style: Style,
  /// Called when the element is pressed with the pointer, or with Enter or
  /// Space while it has focus.
  pub on_press: Option<Callback<()>>,
  /// The element can take keyboard focus, with Tab or a click.
  pub focusable: bool,
  /// Receives keys while the element, or something inside it, has focus.
  pub on_key: Option<KeyHandler>,
  /// Receives the pointer: a press on the element, then every move and the
  /// release until the button is let go, wherever the pointer is.
  pub on_pointer: Option<PointerHandler>,
  /// Called with `true` when the element takes focus and `false` when it
  /// loses it.
  pub on_focus: Option<Callback<bool>>,
  /// A caret and selection to draw over the element's text.
  pub text_cursor: Option<TextCursor>,
  /// What the element is, for assistive tools.
  pub role: Role,
  /// The element takes keyboard focus when it appears (M08).
  pub autofocus: bool,
  /// The element cannot be used now. Controls draw it greyed out; assistive
  /// tools say so.
  pub disabled: bool,
  /// The element's name for assistive tools. Without one, the name comes
  /// from the text inside.
  pub label: Option<String>,
  /// On or off, for a [`Role::Switch`] or [`Role::CheckBox`].
  pub toggled: Option<bool>,
  /// Receives text edits that assistive tools ask for.
  pub on_access: Option<Callback<AccessAction>>,
  /// Receives the mouse wheel and trackpad over the element, before any
  /// scrolling element around it (M09).
  pub on_wheel: Option<Callback<WheelEvent>>,
  /// What the element draws with its own render pass, as a canvas (M09).
  pub canvas: Option<CanvasSource>,
  /// Receives text from an input method while the element has focus. An
  /// element with this takes text input, so the window turns the input
  /// method on while it has focus.
  pub on_ime: Option<Callback<ImeEvent>>,
  /// Receives drags over the element (M10).
  pub on_drop: Option<DropHandler>,
  /// Told the element's size in logical pixels after layout, when it
  /// changes (M10).
  pub on_size: Option<Callback<(f32, f32)>>,
  /// Lets code move focus here with [`FocusTarget::focus`] (M10).
  pub focus_target: Option<FocusTarget>,
  /// The part done, from 0 to 1, for a [`Role::ProgressBar`] (M11).
  pub value: Option<f32>,
  /// The title of the window that shows the element (M11). When several
  /// elements have one, the first in the tree wins.
  pub window_title: Option<String>,
  /// Asks before the window that shows the element closes (M11): the
  /// user's request to close it calls the callback instead, and the
  /// window stays open until [`CloseGuard::close_now`].
  pub close_guard: Option<(CloseGuard, Callback<()>)>,
  /// How the window that shows the element looks (M14). When several
  /// elements have one, the first in the tree wins.
  pub window_style: Option<WindowStyle>,
  /// Called with `true` when the pointer comes onto the element or
  /// something inside it, and `false` when it leaves (M15).
  pub on_hover: Option<Callback<bool>>,
  /// Called with `true` when focus comes into the element or something
  /// inside it, and `false` when it goes out (M15).
  pub on_focus_within: Option<Callback<bool>>,
  /// More about the element than its name, for assistive tools, such as a
  /// tooltip's text (M15). An element with no role gives it to the first
  /// element inside it that has one.
  pub description: Option<String>,
  /// Whether a part that opens and closes, or a list that drops down, is
  /// open (M15).
  pub expanded: Option<bool>,
  /// A check box is neither on nor off, because what it stands for is
  /// partly on (M15).
  pub mixed: bool,
  /// The range [`Element::value`] is in (M15). Without one, the value is
  /// from 0 to 1.
  pub range: Option<ValueRange>,
  /// The text of the element's value, for a [`Role::ComboBox`] (M15).
  pub text_value: Option<String>,
  /// Asked to open a context menu at a point in the window, in logical
  /// pixels (M15): after a right-click on the element or inside it, or the
  /// Menu key or Shift+F10 while focus is inside it.
  pub on_context_menu: Option<Callback<(f32, f32)>>,
  /// Told where a scrolling element is scrolled to, and how much of its
  /// content can be seen, after layout, when either changes (M16).
  pub on_scroll: Option<Callback<Viewport>>,
  /// Asks layout to scroll the element (M16). Layout carries out each new
  /// request once.
  pub scroll_to: Option<ScrollRequest>,
  /// Where an item is in its whole collection, such as a row in a long
  /// list of which only some rows are made (M16).
  pub item: Option<ItemPlace>,
  /// Whether a row, item or cell is selected (M16).
  pub selected: Option<bool>,
  /// The element is the item the keyboard is on inside a view that keeps
  /// focus itself (M16). Assistive tools are told it is the one in focus
  /// while the view has focus.
  pub current: bool,
  /// More than one item inside can be selected (M16).
  pub multiselect: bool,
  /// How many rows and columns a [`Role::Table`] has in all, including
  /// rows that are not made (M16).
  pub table_size: Option<(usize, usize)>,
  /// The column a [`Role::Cell`] or [`Role::ColumnHeader`] is in, from 0
  /// (M16).
  pub column: Option<usize>,
  /// How a table is sorted by this [`Role::ColumnHeader`]'s column (M16).
  pub sorted: Option<SortOrder>,
  /// The views inside.
  pub children: Rc<[View]>,
}

/// Where a scrolling element is scrolled to, and how much can be seen
/// (M16).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Viewport {
  /// How far the content is moved left, in logical pixels.
  pub x: f32,
  /// How far the content is moved up, in logical pixels.
  pub y: f32,
  /// The width of what can be seen: the element inside its border and
  /// padding.
  pub width: f32,
  /// The height of what can be seen.
  pub height: f32,
}

/// A request to scroll an element (M16). Layout carries out each request
/// once, when `serial` is new, so a view can keep giving the same request
/// in later builds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollRequest {
  /// Different for each new request.
  pub serial: u64,
  /// How far across to scroll, or `None` to leave it.
  pub x: Option<f32>,
  /// How far down to scroll, or `None` to leave it.
  pub y: Option<f32>,
}

/// Where an item is in its whole collection (M16).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemPlace {
  /// Its position, from 0.
  pub index: usize,
  /// How many items there are with it: in a tree, how many share its
  /// parent.
  pub count: usize,
  /// How deep it is in a tree, from 1 for the top level.
  pub level: Option<usize>,
}

/// Which way a table is sorted (M16).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SortOrder {
  /// Smallest first.
  Ascending,
  /// Largest first.
  Descending,
}

/// How a window looks where its elements do not say (M14): set by the
/// outermost theme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowStyle {
  /// What the window is filled with.
  pub background: Color,
  /// The keyboard focus ring.
  pub focus_ring: Color,
  /// Selected text in the window in front.
  pub selection: Color,
  /// Selected text in a window in the background.
  pub selection_idle: Color,
  /// The window is dark, so the system draws a dark title bar where it
  /// can.
  pub dark: bool,
  /// The part of a scrollbar that is dragged (M15).
  pub scrollbar_thumb: Color,
  /// What a scrollbar's thumb runs along, shown while the pointer is over
  /// it (M15).
  pub scrollbar_track: Color,
}

impl Default for WindowStyle {
  /// White, with Dovetail's slate for focus and selection: the look
  /// without a theme.
  fn default() -> Self {
    WindowStyle {
      background: Color::WHITE,
      focus_ring: Color::hex(0x33415c),
      selection: Color::hex(0xc5cee0),
      selection_idle: Color::hex(0xdce0e8),
      dark: false,
      scrollbar_thumb: Color::hex(0x8a8f98),
      scrollbar_track: Color { a: 0.06, ..Color::BLACK },
    }
  }
}

impl Element {
  /// An element of this kind with the default style and no children.
  pub fn new(kind: ElementKind) -> Self {
    Self {
      kind,
      style: Style::default(),
      on_press: None,
      focusable: false,
      on_key: None,
      on_pointer: None,
      on_focus: None,
      text_cursor: None,
      on_ime: None,
      role: Role::None,
      autofocus: false,
      disabled: false,
      label: None,
      toggled: None,
      on_access: None,
      on_wheel: None,
      canvas: None,
      on_drop: None,
      on_size: None,
      focus_target: None,
      value: None,
      window_title: None,
      close_guard: None,
      window_style: None,
      on_hover: None,
      on_focus_within: None,
      description: None,
      expanded: None,
      mixed: false,
      range: None,
      text_value: None,
      on_context_menu: None,
      on_scroll: None,
      scroll_to: None,
      item: None,
      selected: None,
      current: false,
      multiselect: false,
      table_size: None,
      column: None,
      sorted: None,
      children: Rc::from([]),
    }
  }

  /// Sets the style.
  pub fn style(
    mut self,
    style: Style,
  ) -> Self {
    self.style = style;
    self
  }

  /// Sets what happens when the element is pressed.
  pub fn on_press(
    mut self,
    on_press: Callback<()>,
  ) -> Self {
    self.on_press = Some(on_press);
    self
  }

  /// Lets the element take keyboard focus.
  pub fn focusable(
    mut self,
    focusable: bool,
  ) -> Self {
    self.focusable = focusable;
    self
  }

  /// Sets what receives keys while the element, or something inside it,
  /// has focus.
  pub fn on_key(
    mut self,
    on_key: KeyHandler,
  ) -> Self {
    self.on_key = Some(on_key);
    self
  }

  /// Sets what receives the pointer.
  pub fn on_pointer(
    mut self,
    on_pointer: PointerHandler,
  ) -> Self {
    self.on_pointer = Some(on_pointer);
    self
  }

  /// Sets what is told when the element takes or loses focus.
  pub fn on_focus(
    mut self,
    on_focus: Callback<bool>,
  ) -> Self {
    self.on_focus = Some(on_focus);
    self
  }

  /// Sets the caret and selection to draw over the element's text.
  pub fn text_cursor(
    mut self,
    cursor: TextCursor,
  ) -> Self {
    self.text_cursor = Some(cursor);
    self
  }

  /// Sets what receives text from an input method.
  pub fn on_ime(
    mut self,
    on_ime: Callback<ImeEvent>,
  ) -> Self {
    self.on_ime = Some(on_ime);
    self
  }

  /// Sets what the element is, for assistive tools.
  pub fn role(
    mut self,
    role: Role,
  ) -> Self {
    self.role = role;
    self
  }

  /// Makes the element take keyboard focus when it appears.
  pub fn autofocus(
    mut self,
    autofocus: bool,
  ) -> Self {
    self.autofocus = autofocus;
    self
  }

  /// Marks the element as not usable now.
  pub fn disabled(
    mut self,
    disabled: bool,
  ) -> Self {
    self.disabled = disabled;
    self
  }

  /// Sets the element's name for assistive tools.
  pub fn label(
    mut self,
    label: impl Into<String>,
  ) -> Self {
    self.label = Some(label.into());
    self
  }

  /// Sets whether a switch or check box is on.
  pub fn toggled(
    mut self,
    on: bool,
  ) -> Self {
    self.toggled = Some(on);
    self
  }

  /// Sets what receives text edits that assistive tools ask for.
  pub fn on_access(
    mut self,
    on_access: Callback<AccessAction>,
  ) -> Self {
    self.on_access = Some(on_access);
    self
  }

  /// Sets what receives the mouse wheel and trackpad.
  pub fn on_wheel(
    mut self,
    on_wheel: Callback<WheelEvent>,
  ) -> Self {
    self.on_wheel = Some(on_wheel);
    self
  }

  /// Makes the element a canvas that draws with `source`.
  pub fn canvas(
    mut self,
    source: CanvasSource,
  ) -> Self {
    self.canvas = Some(source);
    self
  }

  /// Sets what receives drags over the element.
  pub fn on_drop(
    mut self,
    on_drop: DropHandler,
  ) -> Self {
    self.on_drop = Some(on_drop);
    self
  }

  /// Sets what is told the element's size when it changes.
  pub fn on_size(
    mut self,
    on_size: Callback<(f32, f32)>,
  ) -> Self {
    self.on_size = Some(on_size);
    self
  }

  /// Names the element as a place code can move focus to.
  pub fn focus_target(
    mut self,
    target: FocusTarget,
  ) -> Self {
    self.focus_target = Some(target);
    self
  }

  /// Sets the part done, from 0 to 1, for a progress bar.
  pub fn value(
    mut self,
    value: f32,
  ) -> Self {
    self.value = Some(value.clamp(0.0, 1.0));
    self
  }

  /// Sets the title of the window that shows the element.
  pub fn window_title(
    mut self,
    title: impl Into<String>,
  ) -> Self {
    self.window_title = Some(title.into());
    self
  }

  /// Asks `on_request` before the window that shows the element closes.
  /// Call [`CloseGuard::close_now`] to close it.
  pub fn close_guard(
    mut self,
    guard: CloseGuard,
    on_request: Callback<()>,
  ) -> Self {
    self.close_guard = Some((guard, on_request));
    self
  }

  /// Sets how the window that shows the element looks.
  pub fn window_style(
    mut self,
    style: WindowStyle,
  ) -> Self {
    self.window_style = Some(style);
    self
  }

  /// Sets what is told when the pointer comes onto the element and leaves.
  pub fn on_hover(
    mut self,
    on_hover: Callback<bool>,
  ) -> Self {
    self.on_hover = Some(on_hover);
    self
  }

  /// Sets what is told when focus comes into the element and goes out.
  pub fn on_focus_within(
    mut self,
    on_focus_within: Callback<bool>,
  ) -> Self {
    self.on_focus_within = Some(on_focus_within);
    self
  }

  /// Sets more about the element, for assistive tools.
  pub fn description(
    mut self,
    description: impl Into<String>,
  ) -> Self {
    self.description = Some(description.into());
    self
  }

  /// Sets whether the element is open.
  pub fn expanded(
    mut self,
    open: bool,
  ) -> Self {
    self.expanded = Some(open);
    self
  }

  /// Marks a check box as neither on nor off.
  pub fn mixed(
    mut self,
    mixed: bool,
  ) -> Self {
    self.mixed = mixed;
    self
  }

  /// Sets a value in a range, as a slider or number field has. The value
  /// is kept inside the range.
  pub fn value_in(
    mut self,
    value: f32,
    range: ValueRange,
  ) -> Self {
    self.value = Some(value.clamp(range.min, range.max));
    self.range = Some(range);
    self
  }

  /// Sets the text of the element's value.
  pub fn text_value(
    mut self,
    value: impl Into<String>,
  ) -> Self {
    self.text_value = Some(value.into());
    self
  }

  /// Sets what opens a context menu for the element.
  pub fn on_context_menu(
    mut self,
    on_context_menu: Callback<(f32, f32)>,
  ) -> Self {
    self.on_context_menu = Some(on_context_menu);
    self
  }

  /// Sets what is told where the element is scrolled to.
  pub fn on_scroll(
    mut self,
    on_scroll: Callback<Viewport>,
  ) -> Self {
    self.on_scroll = Some(on_scroll);
    self
  }

  /// Asks layout to scroll the element.
  pub fn scroll_to(
    mut self,
    request: ScrollRequest,
  ) -> Self {
    self.scroll_to = Some(request);
    self
  }

  /// Sets where the item is in its whole collection.
  pub fn item(
    mut self,
    place: ItemPlace,
  ) -> Self {
    self.item = Some(place);
    self
  }

  /// Sets whether the item is selected.
  pub fn selected(
    mut self,
    selected: bool,
  ) -> Self {
    self.selected = Some(selected);
    self
  }

  /// Marks the item the keyboard is on.
  pub fn current(
    mut self,
    current: bool,
  ) -> Self {
    self.current = current;
    self
  }

  /// Lets more than one item inside be selected.
  pub fn multiselect(
    mut self,
    multiselect: bool,
  ) -> Self {
    self.multiselect = multiselect;
    self
  }

  /// Sets how many rows and columns a table has in all.
  pub fn table_size(
    mut self,
    rows: usize,
    columns: usize,
  ) -> Self {
    self.table_size = Some((rows, columns));
    self
  }

  /// Sets the column of a cell or header.
  pub fn column(
    mut self,
    column: usize,
  ) -> Self {
    self.column = Some(column);
    self
  }

  /// Sets how the table is sorted by this header's column.
  pub fn sorted(
    mut self,
    order: Option<SortOrder>,
  ) -> Self {
    self.sorted = order;
    self
  }

  /// Sets the children.
  pub fn children(
    mut self,
    children: impl IntoIterator<Item = View>,
  ) -> Self {
    self.children = children.into_iter().collect();
    self
  }
}

impl From<Element> for View {
  fn from(element: Element) -> Self {
    View::Element(Rc::new(element))
  }
}
