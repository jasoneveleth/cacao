//! Wraps `NSTextView` and `UILabel` across platforms, explicitly as a Label.
//! In AppKit, `NSTextField` does double duty, and for clarity we just double
//! the implementation.
//!
//! Labels implement Autolayout, which enable you to specify how things should appear on the screen.
//!
//! ```rust,no_run
//! use cacao::color::rgb;
//! use cacao::layout::{Layout, LayoutConstraint};
//! use cacao::view::Label;
//! use cacao::window::{Window, WindowDelegate};
//!
//! #[derive(Default)]
//! struct AppWindow {
//!     content: Label,
//!     label: Label,
//!     window: Window
//! }
//!
//! impl WindowDelegate for AppWindow {
//!     fn did_load(&mut self, window: Window) {
//!         window.set_minimum_content_size(300., 300.);
//!         self.window = window;
//!
//!         self.label.set_background_color(rgb(224, 82, 99));
//!         self.label.set_text("LOL");
//!         self.content.add_subview(&self.red);
//!
//!         self.window.set_content_view(&self.content);
//!
//!         LayoutConstraint::activate(&[
//!             self.red.top.constraint_equal_to(&self.content.top).offset(16.),
//!             self.red.leading.constraint_equal_to(&self.content.leading).offset(16.),
//!             self.red.trailing.constraint_equal_to(&self.content.trailing).offset(-16.),
//!             self.red.bottom.constraint_equal_to(&self.content.bottom).offset(-16.),
//!         ]);
//!     }
//! }
//! ```
//!
//! For more information on Autolayout, view the module or check out the examples folder.

use objc::runtime::{Class, Object};
use objc::{msg_send, sel, sel_impl};

use crate::color::Color;
use crate::foundation::{id, nil, NSArray, NSString, NO, YES};
use crate::layer::Layer;
use crate::layout::Layout;
use crate::objc_access::ObjcAccess;
use crate::utils::properties::ObjcProperty;

#[cfg(feature = "autolayout")]
use crate::layout::{LayoutAnchorDimension, LayoutAnchorX, LayoutAnchorY, SafeAreaLayoutGuide};

#[cfg(feature = "appkit")]
use crate::pasteboard::PasteboardType;

#[cfg_attr(feature = "appkit", path = "appkit.rs")]
#[cfg_attr(feature = "uikit", path = "uikit.rs")]
mod native_interface;

mod traits;
pub use traits::TextViewDelegate;

pub(crate) static BACKGROUND_COLOR: &str = "cacaoBackgroundColor";
pub(crate) static TEXTVIEW_DELEGATE_PTR: &str = "rstViewDelegatePtr";

#[derive(Debug)]
pub struct TextView<T = ()> {
    /// An internal flag for whether an instance of a TextView<T> is a handle. Typically, there's only
    /// one instance that should have this set to `false` - if that one drops, we need to know to
    /// do some extra cleanup.
    pub is_handle: bool,

    /// A pointer to the Objective-C runtime view controller.
    pub objc: ObjcProperty,

    /// References the underlying layer. This is consistent across AppKit & UIKit - in AppKit
    /// we explicitly opt in to layer backed views.
    pub layer: Layer,

    /// A pointer to the delegate for this view.
    pub delegate: Option<Box<T>>,

    /// A property containing safe layout guides.
    #[cfg(feature = "autolayout")]
    pub safe_layout_guide: SafeAreaLayoutGuide,

    /// A pointer to the Objective-C runtime top layout constraint.
    #[cfg(feature = "autolayout")]
    pub top: LayoutAnchorY,

    /// A pointer to the Objective-C runtime leading layout constraint.
    #[cfg(feature = "autolayout")]
    pub leading: LayoutAnchorX,

    /// A pointer to the Objective-C runtime left layout constraint.
    #[cfg(feature = "autolayout")]
    pub left: LayoutAnchorX,

    /// A pointer to the Objective-C runtime trailing layout constraint.
    #[cfg(feature = "autolayout")]
    pub trailing: LayoutAnchorX,

    /// A pointer to the Objective-C runtime right layout constraint.
    #[cfg(feature = "autolayout")]
    pub right: LayoutAnchorX,

    /// A pointer to the Objective-C runtime bottom layout constraint.
    #[cfg(feature = "autolayout")]
    pub bottom: LayoutAnchorY,

    /// A pointer to the Objective-C runtime width layout constraint.
    #[cfg(feature = "autolayout")]
    pub width: LayoutAnchorDimension,

    /// A pointer to the Objective-C runtime height layout constraint.
    #[cfg(feature = "autolayout")]
    pub height: LayoutAnchorDimension,

    /// A pointer to the Objective-C runtime center X layout constraint.
    #[cfg(feature = "autolayout")]
    pub center_x: LayoutAnchorX,

    /// A pointer to the Objective-C runtime center Y layout constraint.
    #[cfg(feature = "autolayout")]
    pub center_y: LayoutAnchorY
}

impl Default for TextView {
    fn default() -> Self {
        TextView::new()
    }
}

impl TextView {
    /// An internal initializer method for very common things that we need to do, regardless of
    /// what type the end user is creating.
    ///
    /// This handles grabbing autolayout anchor pointers, as well as things related to layering and
    /// so on. It returns a generic `TextView<T>`, which the caller can then customize as needed.
    pub(crate) fn init<T>(view: id) -> TextView<T> {
        unsafe {
            #[cfg(feature = "autolayout")]
            let _: () = msg_send![view, setTranslatesAutoresizingMaskIntoConstraints: NO];

            #[cfg(feature = "appkit")]
            let _: () = msg_send![view, setWantsLayer: YES];
        }

        TextView {
            is_handle: false,
            delegate: None,

            #[cfg(feature = "autolayout")]
            safe_layout_guide: SafeAreaLayoutGuide::new(view),

            #[cfg(feature = "autolayout")]
            top: LayoutAnchorY::top(view),

            #[cfg(feature = "autolayout")]
            left: LayoutAnchorX::left(view),

            #[cfg(feature = "autolayout")]
            leading: LayoutAnchorX::leading(view),

            #[cfg(feature = "autolayout")]
            right: LayoutAnchorX::right(view),

            #[cfg(feature = "autolayout")]
            trailing: LayoutAnchorX::trailing(view),

            #[cfg(feature = "autolayout")]
            bottom: LayoutAnchorY::bottom(view),

            #[cfg(feature = "autolayout")]
            width: LayoutAnchorDimension::width(view),

            #[cfg(feature = "autolayout")]
            height: LayoutAnchorDimension::height(view),

            #[cfg(feature = "autolayout")]
            center_x: LayoutAnchorX::center(view),

            #[cfg(feature = "autolayout")]
            center_y: LayoutAnchorY::center(view),

            layer: Layer::wrap(unsafe { msg_send![view, layer] }),

            objc: ObjcProperty::retain(view)
        }
    }

    /// Returns a default `View`, suitable for customizing and displaying.
    pub fn new() -> Self {
        TextView::init(unsafe { msg_send![native_interface::register_view_class(), new] })
    }
}

impl<T> TextView<T>
where
    T: TextViewDelegate + 'static
{
    /// Initializes a new View with a given `ViewDelegate`. This enables you to respond to events
    /// and customize the view as a module, similar to class-based systems.
    pub fn with(delegate: T) -> TextView<T> {
        let class = native_interface::register_view_class_with_delegate(&delegate);
        let mut delegate = Box::new(delegate);

        let view = unsafe {
            let view: id = msg_send![class, new];
            let ptr = Box::into_raw(delegate);
            (&mut *view).set_ivar(TEXTVIEW_DELEGATE_PTR, ptr as usize);
            delegate = Box::from_raw(ptr);
            view
        };

        let mut view = TextView::init(view);
        (&mut delegate).did_load(view.clone_as_handle());
        view.delegate = Some(delegate);
        view
    }
}

impl<T> TextView<T> {
    /// Returns a clone of this object, sans references to the delegate or
    /// callback pointer. We use this in calling `did_load()` - implementing delegates get a way to
    /// reference, customize and use the view but without the trickery of holding pieces of the
    /// delegate - the `TextView` is the only true holder of those.
    pub fn clone_as_handle(&self) -> TextView {
        TextView {
            delegate: None,
            is_handle: true,
            layer: self.layer.clone(),
            objc: self.objc.clone(),

            #[cfg(feature = "autolayout")]
            safe_layout_guide: self.safe_layout_guide.clone(),

            #[cfg(feature = "autolayout")]
            top: self.top.clone(),

            #[cfg(feature = "autolayout")]
            leading: self.leading.clone(),

            #[cfg(feature = "autolayout")]
            left: self.left.clone(),

            #[cfg(feature = "autolayout")]
            trailing: self.trailing.clone(),

            #[cfg(feature = "autolayout")]
            right: self.right.clone(),

            #[cfg(feature = "autolayout")]
            bottom: self.bottom.clone(),

            #[cfg(feature = "autolayout")]
            width: self.width.clone(),

            #[cfg(feature = "autolayout")]
            height: self.height.clone(),

            #[cfg(feature = "autolayout")]
            center_x: self.center_x.clone(),

            #[cfg(feature = "autolayout")]
            center_y: self.center_y.clone()
        }
    }

    /// Call this to set the background color for the backing layer.
    pub fn set_background_color<C: AsRef<Color>>(&self, color: C) {
        let color: id = color.as_ref().into();

        #[cfg(feature = "appkit")]
        self.objc.with_mut(|obj| unsafe {
            (&mut *obj).set_ivar(BACKGROUND_COLOR, color);
        });

        #[cfg(feature = "uikit")]
        self.objc.with_mut(|obj| unsafe {
            let _: () = msg_send![&*obj, setBackgroundColor: color];
        });
    }
}

impl<T> ObjcAccess for TextView<T> {
    fn with_backing_obj_mut<F: Fn(id)>(&self, handler: F) {
        self.objc.with_mut(handler);
    }

    fn get_from_backing_obj<F: Fn(&Object) -> R, R>(&self, handler: F) -> R {
        self.objc.get(handler)
    }
}

impl<T> Layout for TextView<T> {}

impl<T> Drop for TextView<T> {
    /// If the instance being dropped is _not_ a handle, then we want to go ahead and explicitly
    /// remove it from any super views.
    ///
    /// Why do we do this? It's to try and match Rust's ownership model/semantics. If a Rust value
    /// drops, it (theoretically) makes sense that the View would drop... and not be visible, etc.
    ///
    /// If you're venturing into unsafe code for the sake of custom behavior via the Objective-C
    /// runtime, you can consider flagging your instance as a handle - it will avoid the drop logic here.
    fn drop(&mut self) {
        if !self.is_handle {
            self.remove_from_superview();
        }
    }
}
