use chrono::Utc;
use flowforge_core::hook::StopInput;
use flowforge_core::Result;

pub fn run() -> Result<()> {
    let ctx = super::HookContext::init()?;
    let _input = StopInput::from_value(&ctx.raw)?;

    // End current session if active
    ctx.with_db("end_session", |db| {
        if let Some(session) = db.get_current_session()? {
            db.end_session(&session.id, Utc::now())?;
        }
        Ok(())
    });

    Ok(())
}
