//! Pane command line driver.
//!
//! Applies a scripted sequence of window operations to the engine and prints the
//! resulting tiling. This is a thin shell over the library so what you see on the
//! command line is exactly what the tests exercise.

use std::io::Read;
use std::process::ExitCode;

use pane::manager::Op;
use pane::render::{Renderer, TextRenderer};
use pane::{Rect, WindowManager};

const USAGE: &str = "\
pane - a dependency free tiling window manager engine

USAGE:
    pane [OPTIONS] [SCRIPT...]
    pane demo
    pane --help

OPTIONS:
    --screen WxH    screen size in pixels (default 1200x800)
    --gap N         gap in pixels around each window (default 0)
    --help          print this help

SCRIPT:
    A sequence of operations, separated by ';' or newlines. If no script is given on
    the command line it is read from standard input.

OPERATIONS:
    open [h|v]      open a window, splitting the focused one (default vertical)
    close           close the focused window
    focus <dir>     move focus (left|right|up|down)
    move <dir>      swap the focused window with its neighbour
    resize <dir>    grow the focused window toward a direction
    float           toggle the focused window between tiled and floating
    workspace <n>   switch to workspace n

EXAMPLE:
    pane --gap 8 'open; open h; focus up; open v; resize right'
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<String, String> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return Ok(USAGE.to_string());
    }
    if args.first().map(|s| s.as_str()) == Some("demo") {
        return Ok(demo());
    }

    let mut screen = Rect::new(0, 0, 1200, 800);
    let mut gap = 0i64;
    let mut script_parts: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--screen" => {
                let val = args.get(i + 1).ok_or("--screen needs a value like 1200x800")?;
                screen = parse_screen(val)?;
                i += 2;
            }
            "--gap" => {
                let val = args.get(i + 1).ok_or("--gap needs a value")?;
                gap = val.parse::<i64>().map_err(|_| "gap must be an integer")?;
                i += 2;
            }
            other => {
                script_parts.push(other.to_string());
                i += 1;
            }
        }
    }

    let script = if script_parts.is_empty() {
        let mut s = String::new();
        std::io::stdin()
            .read_to_string(&mut s)
            .map_err(|e| format!("failed to read stdin: {e}"))?;
        s
    } else {
        script_parts.join(" ")
    };

    let ops = parse_script(&script)?;
    let mut wm = WindowManager::new(screen, gap);
    for op in ops {
        wm.apply(op);
    }

    let mut renderer = TextRenderer::new();
    renderer.render(&wm.frame());
    if !wm.report().ok {
        return Err(format!(
            "partition invariant violated: {}",
            wm.report().errors.join("; ")
        ));
    }
    Ok(renderer.buffer)
}

/// Split a script into operations. Commands are separated by ';' or newlines and each
/// command is a whitespace separated token group.
fn parse_script(script: &str) -> Result<Vec<Op>, String> {
    let mut ops = Vec::new();
    for chunk in script.split([';', '\n']) {
        let tokens: Vec<&str> = chunk.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }
        ops.push(Op::parse(&tokens)?);
    }
    Ok(ops)
}

fn parse_screen(val: &str) -> Result<Rect, String> {
    let (w, h) = val
        .split_once(['x', 'X'])
        .ok_or("screen must look like 1200x800")?;
    let w = w.trim().parse::<i64>().map_err(|_| "bad screen width")?;
    let h = h.trim().parse::<i64>().map_err(|_| "bad screen height")?;
    if w <= 0 || h <= 0 {
        return Err("screen dimensions must be positive".to_string());
    }
    Ok(Rect::new(0, 0, w, h))
}

/// A short guided demo that opens several windows, moves focus, resizes, and floats one,
/// printing the tiling and the invariant readout after each step.
fn demo() -> String {
    let steps: &[(&str, &str)] = &[
        ("open", "first window fills the screen"),
        ("open h", "split horizontally, two stacked windows"),
        ("open v", "split the focused one vertically"),
        ("focus up", "move focus to the top window"),
        ("open v", "split the top window"),
        ("resize right", "grow the focused window to the right"),
        ("float", "float the focused window over the tiling"),
    ];

    let mut wm = WindowManager::new(Rect::new(0, 0, 1200, 800), 8);
    let mut out = String::from("pane demo on a 1200x800 screen with an 8px gap\n\n");
    for (script, note) in steps {
        for op in parse_script(script).expect("demo scripts are valid") {
            wm.apply(op);
        }
        out.push_str(&format!("$ {script}    # {note}\n"));
        let mut r = TextRenderer::new();
        r.render(&wm.frame());
        out.push_str(&r.buffer);
        out.push('\n');
    }
    out
}
