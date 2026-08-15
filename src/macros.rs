/// emits typed `&'static str` constants for message keys inside any module.
///
/// use with ordinary `pub mod` for organization — the macro only handles
/// the constant declarations to avoid boilerplate:
///
/// ```rust
/// pub mod keys {
///     pub mod staff {
///         jingle::msg_keys! {
///             PREFIX = "staff.prefix";
///             CHAT_FORMAT = "staff.chat_format";
///         }
///         pub mod freeze {
///             jingle::msg_keys! {
///                 FROZEN = "staff.freeze.frozen";
///             }
///         }
///     }
/// }
/// // usage: keys::staff::PREFIX, keys::staff::freeze::FROZEN
/// ```
#[macro_export]
macro_rules! msg_keys {
    ($($name:ident = $val:literal);+ $(;)?) => {
        $(pub const $name: &'static str = $val;)+
    };
}

#[cfg(test)]
mod tests {
    pub mod keys {
        pub mod staff {
            msg_keys! {
                PREFIX = "staff.prefix";
                CHAT_FORMAT = "staff.chat_format"
            }
            pub mod freeze {
                msg_keys! {
                    FROZEN = "staff.freeze.frozen";
                    UNFROZEN = "staff.freeze.unfrozen"
                }
            }
        }
        pub mod friend {
            pub mod add {
                msg_keys! {
                    REQUEST_SENT = "friend.add.request_sent"
                }
            }
        }
    }

    #[test]
    fn top_level_const() {
        assert_eq!(keys::staff::PREFIX, "staff.prefix");
    }

    #[test]
    fn chat_format_const() {
        assert_eq!(keys::staff::CHAT_FORMAT, "staff.chat_format");
    }

    #[test]
    fn nested_const() {
        assert_eq!(keys::staff::freeze::FROZEN, "staff.freeze.frozen");
        assert_eq!(keys::staff::freeze::UNFROZEN, "staff.freeze.unfrozen");
    }

    #[test]
    fn deep_nested_const() {
        assert_eq!(keys::friend::add::REQUEST_SENT, "friend.add.request_sent");
    }
}
