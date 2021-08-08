//! Wraps `NSView` and `UIView` across platforms.
//!
//! This implementation errs towards the `UIView` side of things, and mostly acts as a wrapper to
//! bring `NSView` to the modern era. It does this by flipping the coordinate system to be what
//! people expect in 2020, and layer-backing all views by default.
//!
//! Views implement Autolayout, which enable you to specify how things should appear on the screen.
//! 
//! ```rust,no_run
//! use cacao::color::rgb;
//! use cacao::layout::{Layout, LayoutConstraint};
//! use cacao::view::View;
//! use cacao::window::{Window, WindowDelegate};
//!
//! #[derive(Default)]
//! struct AppWindow {
//!     content: View,
//!     red: View,
//!     window: Window
//! }
//! 
//! impl WindowDelegate for AppWindow {
//!     fn did_load(&mut self, window: Window) {
//!         window.set_minimum_content_size(300., 300.);
//!         self.window = window;
//!
//!         self.red.set_background_color(rgb(224, 82, 99));
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

use std::collections::HashMap;

use core_graphics::base::CGFloat;
use objc_id::ShareId;
use objc::runtime::{Class, Object};
use objc::{class, msg_send, sel, sel_impl};

use crate::foundation::{id, nil, YES, NO, NSArray, NSString, NSInteger, NSUInteger};
use crate::color::Color;
use crate::layout::{Layout, LayoutAnchorX, LayoutAnchorY, LayoutAnchorDimension};
use crate::scrollview::ScrollView;
use crate::utils::{os, CellFactory, CGSize};
use crate::utils::properties::{ObjcProperty, PropertyNullable};
use crate::view::ViewDelegate;

#[cfg(feature = "appkit")]
use crate::appkit::menu::MenuItem;

#[cfg(feature = "appkit")]
mod appkit;

#[cfg(feature = "appkit")]
use appkit::{register_listview_class, register_listview_class_with_delegate};

//#[cfg(target_os = "ios")]
//mod ios;

//#[cfg(target_os = "ios")]
//use ios::{register_view_class, register_view_class_with_delegate};

mod enums;
pub use enums::{RowAnimation, RowEdge};

mod traits;
pub use traits::ListViewDelegate;

mod row;
pub use row::ListViewRow;

mod actions;
pub use actions::{RowAction, RowActionStyle};

pub(crate) static LISTVIEW_DELEGATE_PTR: &str = "rstListViewDelegatePtr";

use std::any::Any;
use std::sync::{Arc, RwLock};

use std::rc::Rc;
use std::cell::RefCell;

/// A helper method for instantiating view classes and applying default settings to them.
fn common_init(class: *const Class) -> id { 
    unsafe {
        let tableview: id = msg_send![class, new];
        let _: () = msg_send![tableview, setTranslatesAutoresizingMaskIntoConstraints:NO];

        // Let's... make NSTableView into UITableView-ish.
        #[cfg(feature = "appkit")]
        {
            // @TODO: Clean this up in a dealloc method.
            let menu: id = msg_send![class!(NSMenu), new];
            let _: () = msg_send![menu, setDelegate:tableview];
            let _: () = msg_send![tableview, setMenu:menu];

            let _: () = msg_send![tableview, setWantsLayer:YES];
            let _: () = msg_send![tableview, setUsesAutomaticRowHeights:YES];
            let _: () = msg_send![tableview, setFloatsGroupRows:YES];
            //let _: () = msg_send![tableview, setIntercellSpacing:CGSize::new(0., 0.)];
            let _: () = msg_send![tableview, setColumnAutoresizingStyle:1];
            //msg_send![tableview, setSelectionHighlightStyle:-1];
            //let _: () = msg_send![tableview, setAllowsMultipleSelection:NO];
            let _: () = msg_send![tableview, setHeaderView:nil];

            // NSTableView requires at least one column to be manually added if doing so by code.
            let identifier = NSString::no_copy("CacaoListViewColumn");
            let default_column_alloc: id = msg_send![class!(NSTableColumn), new];
            let default_column: id = msg_send![default_column_alloc, initWithIdentifier:&*identifier];
            let _: () = msg_send![default_column, setResizingMask:(1<<0)];
            let _: () = msg_send![tableview, addTableColumn:default_column];
        }

        tableview
    }
}

#[derive(Debug)]
pub struct ListView<T = ()> {
    /// Internal map of cell identifers/vendors. These are used for handling dynamic cell
    /// allocation and reuse, which is necessary for an "infinite" listview.
    cell_factory: CellFactory,

    menu: PropertyNullable<Vec<MenuItem>>,

    /// A pointer to the Objective-C runtime view controller.
    pub objc: ObjcProperty,

    /// In AppKit, we need to manage the NSScrollView ourselves. It's a bit
    /// more old school like that...
    #[cfg(feature = "appkit")]
    pub scrollview: ScrollView,

    /// A pointer to the delegate for this view.
    pub delegate: Option<Box<T>>,

    /// A pointer to the Objective-C runtime top layout constraint.
    pub top: LayoutAnchorY,

    /// A pointer to the Objective-C runtime leading layout constraint.
    pub leading: LayoutAnchorX,

    /// A pointer to the Objective-C runtime left layout constraint.
    pub left: LayoutAnchorX,

    /// A pointer to the Objective-C runtime trailing layout constraint.
    pub trailing: LayoutAnchorX,

    /// A pointer to the Objective-C runtime right layout constraint.
    pub right: LayoutAnchorX,

    /// A pointer to the Objective-C runtime bottom layout constraint.
    pub bottom: LayoutAnchorY,

    /// A pointer to the Objective-C runtime width layout constraint.
    pub width: LayoutAnchorDimension,

    /// A pointer to the Objective-C runtime height layout constraint.
    pub height: LayoutAnchorDimension,

    /// A pointer to the Objective-C runtime center X layout constraint.
    pub center_x: LayoutAnchorX,

    /// A pointer to the Objective-C runtime center Y layout constraint.
    pub center_y: LayoutAnchorY
}

impl Default for ListView {
    fn default() -> Self {
        ListView::new()
    }
}

impl ListView {
    /// @TODO: The hell is this for? 
    pub fn new() -> Self {
        let class = register_listview_class();
        let view = common_init(class);
        
        #[cfg(feature = "appkit")]
        let scrollview = {
            let sview = ScrollView::new();
            
            sview.objc.with_mut(|obj| unsafe {
                let _: () = msg_send![obj, setDocumentView:view];
            });

            sview
        };

        // For AppKit, we need to use the NSScrollView anchor points, not the NSTableView.
        // @TODO: Fix this with proper mutable access.
        #[cfg(feature = "appkit")]
        let anchor_view: id = scrollview.objc.get(|obj| unsafe {
            msg_send![obj, self]
        });
        
        #[cfg(target_os = "ios")]
        let anchor_view: id = view;

        ListView {
            cell_factory: CellFactory::new(),
            menu: PropertyNullable::default(),
            delegate: None,
            top: LayoutAnchorY::top(anchor_view),
            left: LayoutAnchorX::left(anchor_view),
            leading: LayoutAnchorX::leading(anchor_view),
            right: LayoutAnchorX::right(anchor_view),
            trailing: LayoutAnchorX::trailing(anchor_view),
            bottom: LayoutAnchorY::bottom(anchor_view),
            width: LayoutAnchorDimension::width(anchor_view),
            height: LayoutAnchorDimension::height(anchor_view),
            center_x: LayoutAnchorX::center(anchor_view),
            center_y: LayoutAnchorY::center(anchor_view),
            objc: ObjcProperty::retain(view),

            #[cfg(feature = "appkit")]
            scrollview: scrollview
        }
    }
}

impl<T> ListView<T> where T: ListViewDelegate + 'static {
    /// Initializes a new View with a given `ViewDelegate`. This enables you to respond to events
    /// and customize the view as a module, similar to class-based systems.
    pub fn with(delegate: T) -> ListView<T> {
        let class = register_listview_class_with_delegate::<T>(&delegate);
        let view = common_init(class);
        let mut delegate = Box::new(delegate);
        let cell = CellFactory::new();
        
        unsafe {
            //let view: id = msg_send![register_view_class_with_delegate::<T>(), new];
            //let _: () = msg_send![view, setTranslatesAutoresizingMaskIntoConstraints:NO];
            let delegate_ptr: *const T = &*delegate;
            (&mut *view).set_ivar(LISTVIEW_DELEGATE_PTR, delegate_ptr as usize);
            let _: () = msg_send![view, setDelegate:view];
            let _: () = msg_send![view, setDataSource:view];
        };

        #[cfg(feature = "appkit")]
        let scrollview = {
            let sview = ScrollView::new();
            
            sview.objc.with_mut(|obj| unsafe {
                let _: () = msg_send![obj, setDocumentView:view];
            });

            sview
        };

        // For AppKit, we need to use the NSScrollView anchor points, not the NSTableView.
        #[cfg(feature = "appkit")]
        let anchor_view: id = scrollview.objc.get(|obj| unsafe {
            msg_send![obj, self]
        });
        
        //#[cfg(feature = "uikit")]
        //let anchor_view = view;

        let mut view = ListView {
            cell_factory: cell,
            menu: PropertyNullable::default(),
            delegate: None,
            top: LayoutAnchorY::top(anchor_view),
            left: LayoutAnchorX::left(anchor_view),
            leading: LayoutAnchorX::leading(anchor_view),
            right: LayoutAnchorX::right(anchor_view),
            trailing: LayoutAnchorX::trailing(anchor_view),
            bottom: LayoutAnchorY::bottom(anchor_view),
            width: LayoutAnchorDimension::width(anchor_view),
            height: LayoutAnchorDimension::height(anchor_view),
            center_x: LayoutAnchorX::center(anchor_view),
            center_y: LayoutAnchorY::center(anchor_view),
            objc: ObjcProperty::retain(view),
            
            #[cfg(feature = "appkit")]
            scrollview: scrollview
        };

        (&mut delegate).did_load(view.clone_as_handle()); 
        view.delegate = Some(delegate);
        view
    }
}

impl<T> ListView<T> {
    /// An internal method that returns a clone of this object, sans references to the delegate or
    /// callback pointer. We use this in calling `did_load()` - implementing delegates get a way to
    /// reference, customize and use the view but without the trickery of holding pieces of the
    /// delegate - the `View` is the only true holder of those.
    pub(crate) fn clone_as_handle(&self) -> ListView {
        ListView {
            cell_factory: CellFactory::new(),
            menu: self.menu.clone(),
            delegate: None,
            top: self.top.clone(),
            leading: self.leading.clone(),
            left: self.left.clone(),
            trailing: self.trailing.clone(),
            right: self.right.clone(),
            bottom: self.bottom.clone(),
            width: self.width.clone(),
            height: self.height.clone(),
            center_x: self.center_x.clone(),
            center_y: self.center_y.clone(),
            objc: self.objc.clone(),

            #[cfg(feature = "appkit")]
            scrollview: self.scrollview.clone_as_handle()
        }
    }

    /// Register a cell/row vendor function with an identifier. This is stored internally and used
    /// for row-reuse.
    pub fn register<F, R>(&self, identifier: &'static str, vendor: F)
    where
        F: Fn() -> R + 'static,
        R: ViewDelegate + 'static
    {
        self.cell_factory.insert(identifier, vendor);
    }

    /// Dequeue a reusable cell. If one is not in the queue, will create and cache one for reuse.
    pub fn dequeue<R: ViewDelegate + 'static>(&self, identifier: &'static str) -> ListViewRow<R> {
        #[cfg(feature = "appkit")]
        {
            let key = NSString::new(identifier);
            let cell: id = self.objc.get(|obj| unsafe {
                msg_send![obj, makeViewWithIdentifier:&*key owner:nil]
            });
            
            if cell != nil {
                ListViewRow::from_cached(cell)
            } else {
                let delegate: Box<R> = self.cell_factory.get(identifier);
                let view = ListViewRow::with_boxed(delegate);
                view.set_identifier(identifier);
                view
            }
        }
    }

    /// Call this to set the background color for the backing layer.
    pub fn set_background_color<C: AsRef<Color>>(&self, color: C) {
        // @TODO: This is wrong.
        self.objc.with_mut(|obj| unsafe {
            let color = color.as_ref().cg_color();
            let layer: id = msg_send![obj, layer];
            let _: () = msg_send![layer, setBackgroundColor:color];
        });
    }

    /// Sets the style for the underlying NSTableView. This property is only supported on macOS
    /// 11.0+, and will always be `FullWidth` on anything older.
    ///
    /// On non-macOS platforms, this method is a noop.
    #[cfg(feature = "appkit")]
    pub fn set_style(&self, style: crate::foundation::NSInteger) {
        #[cfg(target_os = "macos")]
        if os::is_minimum_version(11) {
            self.objc.with_mut(|obj| unsafe {
                let _: () = msg_send![obj, setStyle:style];
            });
        }
    }

    /// Set whether this control can appear with no row selected.
    ///
    /// This defaults to `true`, but some AppKit pieces (e.g, a sidebar) may want this set to
    /// `false`. This can be particularly useful when implementing a Source List style sidebar
    /// view for navigation purposes.
    #[cfg(feature = "appkit")]
    pub fn set_allows_empty_selection(&self, allows: bool) {
        self.objc.with_mut(|obj| unsafe {
            let _: () = msg_send![obj, setAllowsEmptySelection:match allows {
                true => YES,
                false => NO
            }];
        });
    }

    /// Set the selection highlight style. 
    pub fn set_selection_highlight_style(&self, style: crate::foundation::NSInteger) {
        self.objc.with_mut(|obj| unsafe {
            let _: () = msg_send![obj, setSelectionHighlightStyle:style];
        });
    }

    /// Select the rows at the specified indexes, optionally adding to any existing selections.
    pub fn select_row_indexes(&self, indexes: &[usize], extends_existing: bool) {
        unsafe {
            let index_set: id = msg_send![class!(NSMutableIndexSet), new];

            for index in indexes {
                let _: () = msg_send![index_set, addIndex:index];
            }

            self.objc.with_mut(|obj| {
                let _: () = msg_send![obj, selectRowIndexes:index_set byExtendingSelection:match extends_existing {
                    true => YES,
                    false => NO
                }];
            });
        }
    }

    /// This method should be used when inserting or removing multiple rows at once. Under the
    /// hood, it batches the changes and tries to ensure things are done properly. The provided
    /// `ListView` for the handler is your `ListView`, and you can call `insert_rows`,
    /// `reload_rows`, or `remove_rows` from there.
    ///
    /// ```rust,no_run
    /// list_view.perform_batch_updates(|listview| {
    ///     listview.insert_rows(&[0, 2], RowAnimation::SlideDown);
    /// });
    /// ```
    pub fn perform_batch_updates<F: Fn(ListView)>(&self, update: F) {
        // Note that we need to thread the `with_mut` calls carefully, to avoid deadlocking.
        #[cfg(feature = "appkit")]
        {
            self.objc.get(|obj| unsafe {
                let _: () = msg_send![obj, beginUpdates];
            });
           
            let handle = self.clone_as_handle();
            update(handle);

            // This is cheating, but there's no good way around it at the moment. If we (mutably) lock in
            // Rust here, firing this call will loop back around into `dequeue`, which will then
            // hit a double lock.
            //
            // Personally, I can live with this - `endUpdates` is effectively just flushing the
            // already added updates, so with this small hack here we're able to keep the mutable
            // borrow structure everywhere else, which feels "correct".
            self.objc.get(|obj| unsafe {
                let _: () = msg_send![obj, endUpdates];
            });
        }
    }

    /// Insert new rows at the specified indexes, with the specified animation.
    ///
    /// Your underlying data store must be updated *before* calling this. If inserting multiple
    /// rows at once, you should also run this inside a `perform_batch_updates` call, as that will
    /// optimize things accordingly.
    pub fn insert_rows(&self, indexes: &[usize], animation: RowAnimation) {
        #[cfg(feature = "appkit")]
        unsafe {
            let index_set: id = msg_send![class!(NSMutableIndexSet), new];
            
            for index in indexes {
                let x: NSUInteger = *index as NSUInteger;
                let _: () = msg_send![index_set, addIndex:x];
            }

            let animation_options: NSUInteger = animation.into();

            // We need to temporarily retain this; it can drop after the underlying NSTableView
            // has also retained it.
            let x = ShareId::from_ptr(index_set);
            
            self.objc.with_mut(|obj| {
                let _: () = msg_send![obj, insertRowsAtIndexes:&*x withAnimation:animation_options];
            });
        }
    }

    /// Reload the rows at the specified indexes.
    pub fn reload_rows(&self, indexes: &[usize]) {
        #[cfg(feature = "appkit")]
        unsafe {
            let index_set: id = msg_send![class!(NSMutableIndexSet), new];
            
            for index in indexes {
                let x: NSUInteger = *index as NSUInteger;
                let _: () = msg_send![index_set, addIndex:x];
            }

            let x = ShareId::from_ptr(index_set);

            let ye: id = msg_send![class!(NSIndexSet), indexSetWithIndex:0];
            let y = ShareId::from_ptr(ye);

            // Must use `get` to avoid a double lock.
            self.objc.get(|obj| {
                let _: () = msg_send![obj, reloadDataForRowIndexes:&*x columnIndexes:&*y];
            });
        }
    }

    /// Remove rows at the specified indexes, with the specified animation.
    ///
    /// Your underlying data store must be updated *before* calling this. If removing multiple
    /// rows at once, you should also run this inside a `perform_batch_updates` call, as that will
    /// optimize things accordingly.
    pub fn remove_rows(&self, indexes: &[usize], animations: RowAnimation) {
        #[cfg(feature = "appkit")]
        unsafe {
            let index_set: id = msg_send![class!(NSMutableIndexSet), new];
            
            for index in indexes {
                let x: NSUInteger = *index as NSUInteger;
                let _: () = msg_send![index_set, addIndex:x];
            }

            let animation_options: NSUInteger = animations.into();

            // We need to temporarily retain this; it can drop after the underlying NSTableView
            // has also retained it.
            let x = ShareId::from_ptr(index_set);
            self.objc.with_mut(|obj| {
                let _: () = msg_send![obj, removeRowsAtIndexes:&*x withAnimation:animation_options];
            });
        }
    }

    /// Sets an enforced row-height; if you need dynamic rows, you'll want to
    /// look at ListViewDelegate methods, or use AutoLayout.
    pub fn set_row_height(&self, height: CGFloat) {
        self.objc.with_mut(|obj| unsafe {
            let _: () = msg_send![obj, setRowHeight:height];
        });
    }

    /// This defaults to true. If you're using manual heights, you may want to set this to `false`,
    /// as it will tell AppKit internally to just use the number instead of trying to judge
    /// heights.
    ///
    /// It can make some scrolling situations much smoother.
    pub fn set_uses_automatic_row_heights(&self, uses: bool) {
        #[cfg(feature = "appkit")]
        self.objc.with_mut(|obj| unsafe {
            let _: () = msg_send![obj, setUsesAutomaticRowHeights:match uses {
                true => YES,
                false => NO
            }];
        });
    }

    /// In AppKit, this will instruct the underlying NSTableView to alternate
    /// background colors automatically. If you set this, you possibly want
    /// to hard-set a row height as well.
    pub fn set_uses_alternating_backgrounds(&self, uses: bool) {
        #[cfg(feature = "appkit")]
        self.objc.with_mut(|obj| unsafe {
            let _: () = msg_send![obj, setUsesAlternatingRowBackgroundColors:match uses {
                true => YES,
                false => NO
            }];
        });
    }

    /// End actions for a row. API subject to change.
    pub fn set_row_actions_visible(&self, visible: bool) {
        #[cfg(feature = "appkit")]
        self.objc.with_mut(|obj| unsafe {
            let _: () = msg_send![obj, setRowActionsVisible:match visible {
                true => YES,
                false => NO
            }];
        });
    }

    /// Reloads the underlying ListView. This is more expensive than handling insert/reload/remove
    /// calls yourself, but often easier to implement.
    ///
    /// Calling this will reload (and redraw) your listview based on whatever the data source
    /// reports back.
    pub fn reload(&self) {
        self.objc.with_mut(|obj| unsafe {
            let _: () = msg_send![obj, reloadData];
        });
    }

    /// Returns the selected row.
    pub fn get_selected_row_index(&self) -> NSInteger {
        self.objc.get(|obj| unsafe { msg_send![obj, selectedRow] })
    }
    
    /// Returns the currently clicked row. This is AppKit-specific, and is generally used in context
    /// menu generation to determine what item the context menu should be for. If the clicked area
    /// is not an actual row, this will return `-1`.
    ///
    /// For example (minus the other necessary ListViewDelegate pieces):
    ///
    /// ```rust,no_run
    /// impl ListViewDelegate for MyListView {
    ///     fn context_menu(&self) -> Vec<MenuItem> {
    ///         let clicked_row = self.list_view.get_clicked_row_index();
    ///
    ///         // You could treat this as a "new" menu.
    ///         if clicked_row == -1 {
    ///             return vec![];
    ///         }
    ///
    ///         // User right-clicked on a row, so let's show an edit menu.
    ///         vec![MenuItem::new("Edit")]
    ///     }
    /// }
    /// ```
    pub fn get_clicked_row_index(&self) -> NSInteger {
        self.objc.get(|obj| unsafe { msg_send![obj, clickedRow] })
    }
}

impl<T> Layout for ListView<T> {
    fn with_backing_node<F: Fn(id)>(&self, handler: F) {
        // In AppKit, we need to provide the scrollview for layout purposes - iOS and tvOS will know
        // what to do normally.
        #[cfg(feature = "appkit")]
        self.scrollview.objc.with_mut(handler);
    }

    fn get_from_backing_node<F: Fn(&Object) -> R, R>(&self, handler: F) -> R {
        // In AppKit, we need to provide the scrollview for layout purposes - iOS and tvOS will know
        // what to do normally.
        //
        // @TODO: Review this, as property access isn't really used in the same place as layout
        // stuff... hmm...
        #[cfg(feature = "appkit")]
        self.scrollview.objc.get(handler)
    }
}

impl<T> Drop for ListView<T> {
    /// A bit of extra cleanup for delegate callback pointers. If the originating `View` is being
    /// dropped, we do some logic to clean it all up (e.g, we go ahead and check to see if
    /// this has a superview (i.e, it's in the heirarchy) on the AppKit side. If it does, we go
    /// ahead and remove it - this is intended to match the semantics of how Rust handles things).
    ///
    /// There are, thankfully, no delegates we need to break here.
    fn drop(&mut self) {
        /*if self.delegate.is_some() {
            unsafe {
                let superview: id = msg_send![&*self.objc, superview];
                if superview != nil {
                    let _: () = msg_send![&*self.objc, removeFromSuperview];
                }
            }
        }*/
    }
}
