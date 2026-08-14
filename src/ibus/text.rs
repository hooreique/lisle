use std::collections::HashMap;

use zbus::zvariant::Value;

type Attachments = HashMap<String, Value<'static>>;

fn attachments() -> Attachments {
    HashMap::new()
}

fn attr_list() -> Value<'static> {
    Value::new(("IBusAttrList", attachments(), Vec::<Value<'static>>::new()))
}

pub(super) fn ibus_text(text: impl Into<String>) -> Value<'static> {
    Value::new(("IBusText", attachments(), text.into(), attr_list()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializable_signatures_match_ibus() {
        assert_eq!(attr_list().value_signature().to_string(), "(sa{sv}av)");
        assert_eq!(
            ibus_text("Lisle").value_signature().to_string(),
            "(sa{sv}sv)"
        );
    }
}
