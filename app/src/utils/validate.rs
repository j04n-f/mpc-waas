use actix_web::{Error, error::ErrorUnprocessableEntity, web};
use validator::{Validate, ValidationErrors};

pub fn validate_item<T: Validate>(item: &T) -> Result<(), Error> {
    if let Err(err) = item.validate() {
        let error_messages = format_err(err);
        return Err(ErrorUnprocessableEntity(error_messages));
    }
    Ok(())
}

pub fn validate_req<T: Validate>(json: &web::Json<T>) -> Result<(), Error> {
    validate_item(&json.0)
}

pub fn format_err(validation_errors: ValidationErrors) -> String {
    let error_messages: Vec<String> = validation_errors
        .field_errors()
        .into_iter()
        .map(|(field, errors)| {
            let error_messages: Vec<String> = errors
                .iter()
                .filter_map(|error| error.message.clone())
                .map(|message| message.to_string())
                .collect();

            format!("{}: {}", field, error_messages.join(", "))
        })
        .collect();

    error_messages.join("; ")
}
