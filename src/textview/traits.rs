//! Various traits used for NSTextViews.

use crate::textview::TextView;

/// This trait can be used for implementing custom View behavior. You implement this trait on your
/// struct, and wrap your struct in a `View` or `ViewController`. The view or controller then
/// handles interfacing between your struct and system events.
///
/// It winds up feeling to subclassing, without the ability to subclass multiple levels deep and
/// get ultra confusing.
#[allow(unused_variables)]
pub trait TextViewDelegate {
    /// Used to cache subclass creations on the Objective-C side.
    /// You can just set this to be the name of your view type. This
    /// value *must* be unique per-type.
    const NAME: &'static str;

    /// You should rarely (read: probably never) need to implement this yourself.
    /// It simply acts as a getter for the associated `NAME` const on this trait.
    fn subclass_name(&self) -> &'static str {
        Self::NAME
    }

    /// Called when the TextView is ready to work with. You're passed a `TextView` - this is safe to
    /// store and use repeatedly, but it's not thread safe - any UI calls must be made from the
    /// main thread!
    fn did_load(&mut self, textview: TextView) {}

    /// Posts a notification when the text changes, and forwards the message to the text view’s cell if it responds.
    fn text_did_change(&self, value: &str) {}

    /// Requests permission to begin editing a text object.
    fn text_should_begin_editing(&self, value: &str) -> bool {
        true
    }
}
