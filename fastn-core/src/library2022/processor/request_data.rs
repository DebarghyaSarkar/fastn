pub fn process(
    value: ftd::ast::VariableValue,
    kind: ftd::interpreter::Kind,
    doc: &ftd::interpreter::TDoc,
    config: &fastn_core::Config,
) -> ftd::interpreter::Result<ftd::interpreter::Value> {
    let req = match config.request.as_ref() {
        Some(v) => v,
        None if kind.is_optional() => {
            return Ok(ftd::interpreter::Value::Optional {
                data: Box::new(None),
                kind: kind.into_kind_data(),
            });
        }
        None => {
            return ftd::interpreter::utils::e2(
                "config does not contain http-request object",
                doc.name,
                value.line_number(),
            )
        }
    };
    let mut data = req.query().clone();

    for (name, param_value) in config.named_parameters.iter() {
        let json_value =
            param_value
                .to_serde_value()
                .ok_or(ftd::ftd2021::p1::Error::ParseError {
                    message: format!("ftd value cannot be parsed to json: name: {name}"),
                    doc_id: doc.name.to_string(),
                    line_number: value.line_number(),
                })?;
        data.insert(name.to_string(), json_value);
    }

    match req.body_as_json() {
        Ok(Some(b)) => {
            data.extend(b);
        }
        Ok(None) => {}
        Err(e) => {
            return ftd::interpreter::utils::e2(
                format!("Error while parsing request body: {e:?}"),
                doc.name,
                value.line_number(),
            )
        }
    }

    data.extend(
        config
            .extra_data
            .iter()
            .map(|(k, v)| (k.to_string(), serde_json::Value::String(v.to_string()))),
    );

    doc.from_json(&data, &kind, &value)
}
