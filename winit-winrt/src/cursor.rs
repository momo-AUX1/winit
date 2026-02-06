use windows::UI::Core::CoreCursorType;
use winit_core::cursor::CursorIcon;

pub fn cursor_icon_to_core(icon: CursorIcon) -> CoreCursorType {
    match icon {
        CursorIcon::Default => CoreCursorType::Arrow,
        CursorIcon::Pointer | CursorIcon::Grab | CursorIcon::Grabbing => CoreCursorType::Hand,
        CursorIcon::Text | CursorIcon::VerticalText => CoreCursorType::IBeam,
        CursorIcon::Crosshair => CoreCursorType::Cross,
        CursorIcon::NotAllowed | CursorIcon::NoDrop => CoreCursorType::UniversalNo,
        CursorIcon::Wait | CursorIcon::Progress => CoreCursorType::Wait,
        CursorIcon::Move | CursorIcon::AllResize => CoreCursorType::SizeAll,
        CursorIcon::EResize
        | CursorIcon::WResize
        | CursorIcon::EwResize
        | CursorIcon::ColResize => CoreCursorType::SizeWestEast,
        CursorIcon::NResize
        | CursorIcon::SResize
        | CursorIcon::NsResize
        | CursorIcon::RowResize => CoreCursorType::SizeNorthSouth,
        CursorIcon::NeswResize | CursorIcon::NeResize | CursorIcon::SwResize => {
            CoreCursorType::SizeNortheastSouthwest
        },
        CursorIcon::NwseResize | CursorIcon::NwResize | CursorIcon::SeResize => {
            CoreCursorType::SizeNorthwestSoutheast
        },
        CursorIcon::Help => CoreCursorType::Help,
        _ => CoreCursorType::Arrow,
    }
}
