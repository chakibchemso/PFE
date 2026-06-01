use oxivgl::widgets::*;

pub fn create(parent: &impl AsLvHandle) -> Result<(), WidgetError> {
    let lbl = Label::new(parent)?;
    lbl.text("ECG").align(oxivgl::widgets::Align::Center, 0, 0);
    let _ = Child::new(lbl);
    Ok(())
}
