use tray_icon::Icon;

#[derive(Clone, Copy)]
pub enum Color {
    Green,
    Red,
}

impl Color {
    fn rgb(self) -> (u8, u8, u8) {
        match self {
            Color::Green => (46, 204, 113),
            Color::Red => (231, 76, 60),
        }
    }
}

/// Ícone com uma cor só (os dois protocolos no mesmo estado) -- por baixo,
/// só um caso particular do ícone dividido, com os dois lados iguais.
pub fn make_icon(color: Color) -> anyhow::Result<Icon> {
    make_icon_dividido(color, color)
}

/// Gera um círculo dividido ao meio (esquerda/direita), cada lado com sua
/// cor -- usado pra sinalizar o status de HTTP (esquerda) e HTTPS
/// (direita) num ícone só, inclusive quando só um dos dois está
/// funcionando (ex.: esquerda verde, direita vermelha).
pub fn make_icon_dividido(esquerda: Color, direita: Color) -> anyhow::Result<Icon> {
    const SIZE: u32 = 32;
    let (re, ge, be) = esquerda.rgb();
    let (rd, gd, bd) = direita.rgb();

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
                let (r, g, b) = if (x as f32 + 0.5) < center {
                    (re, ge, be)
                } else {
                    (rd, gd, bd)
                };
                rgba[idx] = r;
                rgba[idx + 1] = g;
                rgba[idx + 2] = b;
                rgba[idx + 3] = 255;
            }
        }
    }

    Ok(Icon::from_rgba(rgba, SIZE, SIZE)?)
}
