use tray_icon::Icon;

#[derive(Clone, Copy)]
pub enum Color {
    Green,
    Red,
}

/// Gera um ícone (círculo colorido) em tempo de execução, sem precisar
/// de arquivo .ico embutido no pacote.
pub fn make_icon(color: Color) -> anyhow::Result<Icon> {
    const SIZE: u32 = 32;
    let (r, g, b) = match color {
        Color::Green => (46u8, 204, 113),
        Color::Red => (231u8, 76, 60),
    };

    let center = SIZE as f32 / 2.0;
    let radius = SIZE as f32 / 2.0 - 2.0;
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];

    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 + 0.5 - center;
            let dy = y as f32 + 0.5 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= radius {
                let idx = ((y * SIZE + x) * 4) as usize;
                rgba[idx] = r;
                rgba[idx + 1] = g;
                rgba[idx + 2] = b;
                rgba[idx + 3] = 255;
            }
        }
    }

    Ok(Icon::from_rgba(rgba, SIZE, SIZE)?)
}
