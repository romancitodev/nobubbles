/// UI components for building terminal interfaces.
mod input;
mod text;

pub use input::Input;
pub use text::Text;

/// Internal representation of a UI element.
/// Not yet used - part of planned component system.
struct Element {
    id: u32,
    content: String,
}

/// Convert something into a renderable element.
/// Not implemented yet.
pub trait IntoElement {
    fn into_element(self) -> Element;
}

// struct App;

// pub fn app() -> Result<..., ...> {
//     let renderer = Renderer::<backend::Terminal>::new();
//     renderer.draw(|f: &mut Frame, ctx: Ctx<App>| {
//         ctx.steps.render(&mut frame, ctx); // all the steps now how to render themselves
//     })
//     renderer.run(AppState::new());
// }
