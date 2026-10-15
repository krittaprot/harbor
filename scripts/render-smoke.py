# /// script
# dependencies = ["playwright"]
# ///
"""Render DOM exported by native WebKit. No server and no fixture UI data."""
from pathlib import Path
from playwright.sync_api import sync_playwright

root = Path(__file__).resolve().parent.parent
source = Path.home() / "Library/Application Support/Harbor/diagnostics"
out = root / "docs/verification"
out.mkdir(parents=True, exist_ok=True)
with sync_playwright() as p:
    browser = p.chromium.launch(executable_path="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome", headless=True)
    page = browser.new_page(viewport={"width": 1440, "height": 1000}, device_scale_factor=1)
    page.route("**/*", lambda route: route.abort())
    for stage in ["light", "dark", "restart", "adopt", "login", "popover"]:
        page.set_viewport_size({"width": 360 if stage == "popover" else 1440, "height": 600 if stage == "popover" else 1000})
        html = (source / f"native-{stage}.html").read_text()
        page.set_content(html)
        page.wait_for_timeout(600)
        page.screenshot(path=str(out / f"harbor-{stage}.png"), full_page=False)
    html = (source / "native-light.html").read_text()
    page.set_content(html)
    for width in [320, 375, 414, 768, 1100, 1440]:
        page.set_viewport_size({"width": width, "height": 900})
        overflow = page.evaluate("document.documentElement.scrollWidth > innerWidth")
        assert not overflow, f"Page overflows at {width}px"
        print(f"Layout {width}px: no page overflow")
    browser.close()
