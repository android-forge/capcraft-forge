# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 CapCraft contributors
"""Our own shapes for video: bursts, badges, ribbons, bubbles, arrows, frames, marks and the like."""
from math import cos, pi, sin

INK, PAPER = "#161616", "#ffffff"
COLOURS = [("red", "#ff3b30"), ("yellow", "#ffd60a"), ("blue", "#2979ff"), ("green", "#00c853"), ("pink", "#ff4081"), ("violet", "#7c4dff"), ("white", "#ffffff"), ("black", "#161616")]


def star(points, outer, inner, cx=256, cy=256, turn=-pi / 2):
    p = []
    for i in range(points * 2):
        r = outer if i % 2 == 0 else inner
        a = turn + i * pi / points
        p.append(f"{cx + r * cos(a):.1f} {cy + r * sin(a):.1f}")
    return "M" + "L".join(p) + "Z"


def shapes():
    """Yields (name, words, width, height, body)."""
    for points, inner, label in ((8, 170, "badge"), (12, 190, "burst"), (16, 205, "burst"), (24, 215, "seal"), (5, 110, "star"), (4, 70, "sparkle"), (6, 130, "star")):
        for colour, hexa in COLOURS[:6]:
            rim = PAPER if label != "sparkle" else "none"
            yield f"{label}-{points}-{colour}", f"{label} sale offer price tag {colour} shape explosion pow", 512, 512, f'<path d="{star(points, 240, inner)}" fill="{hexa}" stroke="{rim}" stroke-width="10" stroke-linejoin="round"/>'
    for colour, hexa in COLOURS:
        edge = INK if colour in ("white", "yellow") else PAPER
        yield f"ribbon-{colour}", f"ribbon banner label title {colour}", 640, 200, f'<path d="M0 20H640L580 100L640 180H0L60 100Z" fill="{hexa}" stroke="{edge}" stroke-width="8" stroke-linejoin="round"/>'
        yield f"lower-third-{colour}", f"lower third title name bar caption news {colour}", 900, 180, f'<path d="M0 0H900V180H0Z" fill="{hexa}" opacity=".88"/><path d="M0 0H22V180H0Z" fill="{edge}"/>'
        yield f"pill-{colour}", f"pill button capsule {colour}", 640, 180, f'<rect x="6" y="6" width="628" height="168" rx="84" fill="{hexa}" stroke="{edge}" stroke-width="10"/>'
        yield f"speech-bubble-{colour}", f"speech bubble talk chat comment say {colour}", 560, 460, f'<path d="M90 10H470A80 80 0 0 1 550 90V260A80 80 0 0 1 470 340H240L130 450V340H90A80 80 0 0 1 10 260V90A80 80 0 0 1 90 10Z" fill="{hexa}" stroke="{edge}" stroke-width="12" stroke-linejoin="round"/>'
        yield f"thought-bubble-{colour}", f"thought bubble think idea dream cloud {colour}", 560, 480, f'<path d="M150 90A105 105 0 0 1 345 70A95 95 0 0 1 500 165A90 90 0 0 1 440 330A120 120 0 0 1 225 345A100 100 0 0 1 60 270A100 100 0 0 1 150 90Z" fill="{hexa}" stroke="{edge}" stroke-width="12"/><circle cx="130" cy="390" r="34" fill="{hexa}" stroke="{edge}" stroke-width="10"/><circle cx="72" cy="446" r="20" fill="{hexa}" stroke="{edge}" stroke-width="9"/>'
        stroke = f'fill="none" stroke="{hexa}" stroke-linecap="round" stroke-linejoin="round"'
        yield f"arrow-curved-{colour}", f"arrow curved point hand drawn {colour}", 512, 400, f'<path d="M40 330C60 120 260 40 440 150M440 150L330 130M440 150L420 40" {stroke} stroke-width="34"/>'
        yield f"arrow-loop-{colour}", f"arrow loop swirl hand drawn {colour}", 620, 380, f'<path d="M30 300C180 330 330 250 300 130C280 60 190 80 210 160C235 260 420 270 580 190M580 190L475 165M580 190L510 280" {stroke} stroke-width="28"/>'
        yield f"arrow-bold-{colour}", f"arrow bold big pointer right {colour}", 560, 360, f'<path d="M20 120H300V20L540 180L300 340V240H20Z" fill="{hexa}" stroke="{edge}" stroke-width="14" stroke-linejoin="round"/>'
        yield f"arrow-down-{colour}", f"arrow down swipe scroll below link {colour}", 320, 520, f'<path d="M160 30V470M160 470L50 350M160 470L270 350" {stroke} stroke-width="44"/>'
        yield f"circle-drawn-{colour}", f"circle hand drawn ring mark highlight round {colour}", 620, 420, f'<path d="M420 40C220 -10 20 90 30 220C40 370 260 410 420 380C580 345 620 200 540 110C470 40 320 30 210 62" {stroke} stroke-width="24"/>'
        yield f"underline-{colour}", f"underline stroke brush highlight swoosh {colour}", 640, 120, f'<path d="M20 80C180 30 420 25 620 50C460 50 300 65 170 100" {stroke} stroke-width="24"/>'
        yield f"highlight-{colour}", f"highlight marker strip text behind {colour}", 640, 160, f'<path d="M12 40C200 18 440 22 630 14L620 128C420 140 200 138 18 148Z" fill="{hexa}" opacity=".75"/>'
        yield f"cross-out-{colour}", f"cross out scribble strike wrong x {colour}", 460, 460, f'<path d="M40 50C170 170 290 290 420 415M425 45C300 160 170 290 45 420" {stroke} stroke-width="46"/>'
        yield f"tick-drawn-{colour}", f"tick check correct yes done hand drawn {colour}", 520, 420, f'<path d="M40 220C100 260 150 310 190 380C260 230 360 110 485 35" {stroke} stroke-width="50"/>'
        yield f"frame-corners-{colour}", f"frame corners viewfinder focus camera brackets {colour}", 600, 600, f'<path d="M20 170V20H170M430 20H580V170M580 430V580H430M170 580H20V430" {stroke} stroke-width="22"/>'
        yield f"frame-round-{colour}", f"frame border rounded rectangle {colour}", 600, 800, f'<rect x="14" y="14" width="572" height="772" rx="60" {stroke} stroke-width="20"/>'
        yield f"frame-polaroid-{colour}", f"frame polaroid photo instant {colour}", 600, 720, f'<path fill-rule="evenodd" d="M0 0H600V720H0ZM40 40V560H560V40Z" fill="{hexa}"/>'
        yield f"brackets-{colour}", f"brackets quote focus {colour}", 700, 300, f'<path d="M110 20H30V280H110M590 20H670V280H590" {stroke} stroke-width="24"/>'
        yield f"speed-lines-{colour}", f"speed lines motion fast comic {colour}", 640, 360, f'<path d="M30 60H420M120 130H610M20 200H500M160 270H580M60 340H380" {stroke} stroke-width="18"/>'
        rays = "".join(f"M{256 + 150 * cos(i * pi / 8):.1f} {256 + 150 * sin(i * pi / 8):.1f}L{256 + 240 * cos(i * pi / 8):.1f} {256 + 240 * sin(i * pi / 8):.1f}" for i in range(16))
        yield f"rays-{colour}", f"rays shine pop sunburst attention {colour}", 512, 512, f'<path d="{rays}" {stroke} stroke-width="22"/>'
        focus = "".join(f"M{256 + 250 * cos(i * pi / 30):.1f} {256 + 250 * sin(i * pi / 30):.1f}L{256 + (110 + 45 * ((i * 7) % 3)) * cos((i + .5) * pi / 30):.1f} {256 + (110 + 45 * ((i * 7) % 3)) * sin((i + .5) * pi / 30):.1f}L{256 + 250 * cos((i + .62) * pi / 30):.1f} {256 + 250 * sin((i + .62) * pi / 30):.1f}Z" for i in range(0, 60))
        yield f"focus-lines-{colour}", f"focus lines manga comic zoom shock {colour}", 512, 512, f'<path d="{focus}" fill="{hexa}"/>'
        wave = "M20 100" + "".join(f"q35 {-90 if i % 2 == 0 else 90} 70 0" for i in range(8))
        yield f"wave-{colour}", f"wave squiggle line divider {colour}", 600, 200, f'<path d="{wave}" {stroke} stroke-width="22"/>'
        dots = "".join(f'<circle cx="{40 + 70 * x}" cy="{40 + 70 * y}" r="14"/>' for x in range(6) for y in range(4))
        yield f"dots-{colour}", f"dots pattern grid halftone {colour}", 430, 290, f'<g fill="{hexa}">{dots}</g>'
        tape = "M0 18L14 0L28 18L42 0L56 18L70 0"
        yield f"tape-{colour}", f"tape sticky washi paper strip {colour}", 520, 150, f'<path d="M12 20L508 4L518 124L22 144Z" fill="{hexa}" opacity=".8"/>'
        yield f"brush-stroke-{colour}", f"brush stroke paint splash background {colour}", 700, 220, f'<path d="M30 70C120 20 260 50 380 30C500 12 640 20 680 50C600 60 690 80 640 100C690 120 600 130 660 150C560 190 400 170 280 190C160 208 60 196 20 170C70 160 10 140 60 122C10 108 60 90 30 70Z" fill="{hexa}"/>'
    mono = f'fill="none" stroke="{PAPER}" stroke-width="10" stroke-linecap="round"'
    yield "rec-viewfinder", "rec recording viewfinder camera frame hud", 900, 600, f'<path d="M30 130V30H180M720 30H870V130M870 470V570H720M180 570H30V470" {mono}/><circle cx="90" cy="90" r="22" fill="#ff3b30"/><path d="M780 72H840V108H780ZM840 82H852V98H840Z" fill="none" stroke="{PAPER}" stroke-width="6"/><path d="M786 78H820V102H786Z" fill="{PAPER}"/>'
    yield "play-button", "play button video start", 400, 400, f'<circle cx="200" cy="200" r="190" fill="{INK}" opacity=".6"/><path d="M150 115L300 200L150 285Z" fill="{PAPER}"/>'
    yield "player-bar", "player progress bar music video seek timeline", 900, 120, f'<path d="M20 60H880" {mono}/><path d="M20 60H420" fill="none" stroke="#ff3b30" stroke-width="12" stroke-linecap="round"/><circle cx="420" cy="60" r="24" fill="#ff3b30"/>'
    yield "loading-bar", "loading bar progress wait", 800, 120, f'<rect x="10" y="20" width="780" height="80" rx="40" fill="none" stroke="{PAPER}" stroke-width="12"/><rect x="30" y="40" width="480" height="40" rx="20" fill="#00c853"/>'
    yield "battery-low", "battery low charge phone", 560, 280, f'<rect x="10" y="20" width="490" height="240" rx="40" fill="none" stroke="{PAPER}" stroke-width="18"/><path d="M520 100H550V180H520Z" fill="{PAPER}"/><rect x="45" y="55" width="95" height="170" rx="16" fill="#ff3b30"/>'
    yield "search-bar", "search bar google type find box", 900, 160, f'<rect x="8" y="8" width="884" height="144" rx="72" fill="{PAPER}" stroke="#dadce0" stroke-width="6"/><circle cx="92" cy="72" r="28" fill="none" stroke="#5f6368" stroke-width="12"/><path d="M114 94L140 122" fill="none" stroke="#5f6368" stroke-width="14" stroke-linecap="round"/>'
    yield "notification-card", "notification message card phone popup", 900, 220, f'<rect x="6" y="6" width="888" height="208" rx="48" fill="{PAPER}" opacity=".92"/><rect x="44" y="50" width="120" height="120" rx="30" fill="#00c853"/><path d="M200 84H420M200 140H800" fill="none" stroke="#9e9e9e" stroke-width="20" stroke-linecap="round"/>'
    yield "split-line", "split divider line before after compare", 60, 900, f'<path d="M30 0V900" fill="none" stroke="{PAPER}" stroke-width="10"/><circle cx="30" cy="450" r="28" fill="{PAPER}"/>'
    yield "letterbox", "letterbox cinema bars film black", 900, 1600, f'<path d="M0 0H900V190H0ZM0 1410H900V1600H0Z" fill="#000"/>'
    yield "film-strip", "film strip movie reel frame", 900, 260, '<path fill-rule="evenodd" d="M0 0H900V260H0Z' + "".join(f"M{20 + 60 * i} 16h30v30h-30ZM{20 + 60 * i} 214h30v30h-30Z" for i in range(15)) + "".join(f"M{20 + 290 * i} 66h270v128h-270Z" for i in range(3)) + '" fill="#111"/>'
    yield "grid-thirds", "grid rule of thirds guide overlay", 900, 900, f'<path d="M300 0V900M600 0V900M0 300H900M0 600H900" fill="none" stroke="{PAPER}" stroke-width="4" opacity=".7"/>'
    yield "crosshair", "crosshair target aim scope", 500, 500, f'<circle cx="250" cy="250" r="170" fill="none" stroke="#ff3b30" stroke-width="12"/><path d="M250 10V180M250 320V490M10 250H180M320 250H490" fill="none" stroke="#ff3b30" stroke-width="12"/><circle cx="250" cy="250" r="10" fill="#ff3b30"/>'
    pieces = ""
    for i in range(70):
        x, y = (i * 137.5) % 780 + 10, (i * i * 37.7) % 880 + 10
        c = COLOURS[i % 6][1]
        pieces += f'<rect x="{x:.0f}" y="{y:.0f}" width="{14 + i % 3 * 6}" height="{22 + i % 4 * 5}" rx="3" fill="{c}" transform="rotate({i * 53 % 180} {x:.0f} {y:.0f})"/>'
    yield "confetti", "confetti party celebrate birthday congratulations", 800, 900, pieces
    hearts = ""
    for i in range(14):
        x, y, s = (i * 173) % 560 + 40, (i * 241) % 760 + 40, .5 + (i * 7 % 5) * .22
        hearts += f'<path transform="translate({x} {y}) scale({s:.2f})" d="M0 60C-90 0-60-70 0-30C60-70 90 0 0 60Z" fill="{["#ff1744", "#ff80ab", "#f50057"][i % 3]}"/>'
    yield "floating-hearts", "hearts floating love like reactions", 640, 880, hearts
    sparkles = "".join(f'<path d="{star(4, 22 + i * 13 % 46, 5 + i % 4, (i * 167) % 560 + 40, (i * 229) % 560 + 40)}" fill="#fff3b0"/>' for i in range(18))
    yield "sparkles-scatter", "sparkles glitter shine stars magic", 640, 640, sparkles
