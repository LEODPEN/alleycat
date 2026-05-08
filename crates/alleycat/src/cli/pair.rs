use clap::Args;
use qrcodegen::{QrCode, QrCodeEcc};

use crate::cli;
use crate::daemon::control::Request;
use crate::host;
use crate::ipc;
use crate::protocol::PairPayload;

#[derive(Args, Debug)]
pub struct PairArgs {
    /// Render an ASCII QR code for the pair payload.
    #[arg(long)]
    pub qr: bool,
}

pub async fn run(args: PairArgs) -> anyhow::Result<()> {
    cli::ensure_current_daemon().await?;
    let payload: PairPayload = if ipc::is_daemon_running().await {
        let resp = cli::send(Request::Pair).await?;
        cli::decode_data(resp)?
    } else {
        let cfg = crate::config::load_or_init().await?;
        let secret_key = crate::state::load_or_create_secret_key().await?;
        host::pair_payload(&secret_key, &cfg, None)
    };

    let json = serde_json::to_string(&payload)?;
    println!("{json}");
    if args.qr {
        println!();
        print_qr(&json)?;
    }
    Ok(())
}

fn print_qr(data: &str) -> anyhow::Result<()> {
    // Low ECC over Medium: ~7% capacity loss vs ~15%, often shaves one
    // version off the matrix. The QR is rendered on a clean digital screen
    // for a phone camera at close range — there's no dirt/glare to recover
    // from, so the higher levels are wasted bits.
    let code = QrCode::encode_text(data, QrCodeEcc::Low)
        .map_err(|err| anyhow::anyhow!("encoding QR: {err:?}"))?;
    let size = code.size();
    let border = 2_i32;
    let lo = -border;
    let hi = size + border;

    // Render two QR rows per terminal row using upper/lower half-block
    // glyphs (U+2580 ▀, U+2584 ▄, U+2588 █). Halves the vertical size of
    // the rendered code; combined with one-cell-per-module width, the QR
    // ends up roughly square in normal terminal aspect ratios.
    let module = |x: i32, y: i32| -> bool {
        if y < 0 || y >= size {
            false
        } else {
            code.get_module(x, y)
        }
    };
    let mut y = lo;
    while y < hi {
        let mut line = String::with_capacity((hi - lo) as usize);
        for x in lo..hi {
            let top = module(x, y);
            let bot = module(x, y + 1);
            line.push(match (top, bot) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            });
        }
        println!("{line}");
        y += 2;
    }
    Ok(())
}
