//! Encode Primer key events for a PTY, keeping editor shortcuts separate.
use helix_view::{input::KeyEvent, keyboard::{KeyCode, KeyModifiers}};

/// Encode a key for a traditional VT-compatible application.
/// Unsupported keys return None rather than emitting an incorrect sequence.
pub fn encode_key(key: KeyEvent) -> Option<Vec<u8>> {
    use KeyCode::*;
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let mut bytes = match key.code {
        Char(c) if ctrl => {
            let lower = c.to_ascii_lowercase();
            match lower {
                'a'..='z' => vec![lower as u8 - b'a' + 1],
                '@' | ' ' => vec![0],
                '[' => vec![27],
                '\\' => vec![28],
                ']' => vec![29],
                '^' => vec![30],
                '_' => vec![31],
                '?' => vec![127],
                _ => return None,
            }
        }
        Char(c) => {
            let mut buf = [0; 4];
            c.encode_utf8(&mut buf).as_bytes().to_vec()
        }
        Enter => vec![b'\r'],
        Tab => vec![b'\t'],
        Backspace => vec![127],
        Esc => vec![27],
        Up => b"\x1b[A".to_vec(),
        Down => b"\x1b[B".to_vec(),
        Right => b"\x1b[C".to_vec(),
        Left => b"\x1b[D".to_vec(),
        Home => b"\x1b[H".to_vec(),
        End => b"\x1b[F".to_vec(),
        Delete => b"\x1b[3~".to_vec(),
        Insert => b"\x1b[2~".to_vec(),
        PageUp => b"\x1b[5~".to_vec(),
        PageDown => b"\x1b[6~".to_vec(),
        F(n @ 1..=4) => vec![27, b'O', b'P' + (n - 1)],
        F(n @ 5..=12) => {
            let code = [15, 17, 18, 19, 20, 21, 23, 24][(n - 5) as usize];
            format!("\x1b[{code}~").into_bytes()
        }
        _ => return None,
    };
    if alt {
        bytes.insert(0, 27);
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent { code, modifiers }
    }

    #[test]
    fn encodes_text_and_control_keys() {
        assert_eq!(encode_key(key(KeyCode::Char('é'), KeyModifiers::NONE)), Some("é".as_bytes().to_vec()));
        assert_eq!(encode_key(key(KeyCode::Char('c'), KeyModifiers::CONTROL)), Some(vec![3]));
        assert_eq!(encode_key(key(KeyCode::Char('\\'), KeyModifiers::CONTROL)), Some(vec![28]));
        assert_eq!(encode_key(key(KeyCode::Up, KeyModifiers::NONE)), Some(b"\x1b[A".to_vec()));
        assert_eq!(encode_key(key(KeyCode::Char('x'), KeyModifiers::ALT)), Some(vec![27, b'x']));
    }
}
