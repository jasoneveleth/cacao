//! This module is a thin wrapper over the NSTextView class

use std::sync::Once;

use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel, BOOL};
use objc::{class, msg_send, sel, sel_impl};
use objc_id::Id;

use crate::foundation::{id, load_or_register_class, nil, NSUInteger, NO, YES};
use crate::utils::load;
use crate::textview::{TextViewDelegate, BACKGROUND_COLOR, TEXTVIEW_DELEGATE_PTR};
use crate::textview::NSString;

/// Called for layer updates.
extern "C" fn update_layer(this: &Object, _: Sel) {
    unsafe {
        let background_color: id = *this.get_ivar(BACKGROUND_COLOR);

        if background_color != nil {
            let layer: id = msg_send![this, layer];
            let cg: id = msg_send![background_color, CGColor];
            let _: () = msg_send![layer, setBackgroundColor: cg];
        }
    }
}

extern "C" fn text_did_change<T: TextViewDelegate>(this: &mut Object, _: Sel, _info: id) {
    let textview = load::<T>(this, TEXTVIEW_DELEGATE_PTR);
    let s = NSString::retain(unsafe { msg_send![this, stringValue] });
    textview.text_did_change(s.to_str());
}

extern "C" fn text_should_begin_editing<T: TextViewDelegate>(this: &mut Object, _: Sel, _info: id) -> BOOL {
    let textview = load::<T>(this, TEXTVIEW_DELEGATE_PTR);
    let s = NSString::retain(unsafe { msg_send![this, stringValue] });

    match textview.text_should_begin_editing(s.to_str()) {
        true => YES,
        false => NO
    }
}


/// Injects an `NSTextView` subclass. This is used for the default textviews that don't use delegates - we
/// have separate classes here since we don't want to waste cycles on methods that will never be
/// used if there's no delegates.
pub(crate) fn register_textview_class() -> *const Class {
    static mut TEXTVIEW_CLASS: *const Class = 0 as *const Class;
    static INIT: Once = Once::new();

    INIT.call_once(|| unsafe {
        let superclass = class!(NSTextView);
        let mut decl = ClassDecl::new("RSTTextView", superclass).unwrap();

        decl.add_ivar::<id>(BACKGROUND_COLOR);

        TEXTVIEW_CLASS = decl.register();
    });

    unsafe { TEXTVIEW_CLASS }
}

/// Injects an `NSTextView` subclass, with some callback and pointer ivars for what we
/// need to do.
pub(crate) fn register_textview_class_with_delegate<T: TextViewDelegate>(instance: &T) -> *const Class {
    load_or_register_class("NSTextView", instance.subclass_name(), |decl| unsafe {
        // A pointer to the TextViewDelegate instance on the Rust side.
        // It's expected that this doesn't move.
        decl.add_ivar::<usize>(TEXTVIEW_DELEGATE_PTR);
        decl.add_ivar::<id>(BACKGROUND_COLOR);

        decl.add_method(sel!(textDidChange:), text_did_change::<T> as extern "C" fn(&mut Object, _, _));
    })
}
