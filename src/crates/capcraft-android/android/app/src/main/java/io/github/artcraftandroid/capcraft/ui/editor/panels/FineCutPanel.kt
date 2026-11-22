package io.github.artcraftandroid.capcraft.ui.editor.panels

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.artcraftandroid.capcraft.core.ClipBox
import io.github.artcraftandroid.capcraft.core.FineCut
import io.github.artcraftandroid.capcraft.core.releaseFine
import io.github.artcraftandroid.capcraft.engine.Threads
import io.github.artcraftandroid.capcraft.engine.intoBitmap
import io.github.artcraftandroid.capcraft.engine.userMessage
import io.github.artcraftandroid.capcraft.ui.Palette
import io.github.artcraftandroid.capcraft.ui.Type
import io.github.artcraftandroid.capcraft.ui.editor.EditorViewModel
import kotlin.math.abs
import kotlin.math.roundToInt
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** How the fine cutout is set while its panel is open, and what it would leave. */
private object Fine {
    var detail by mutableFloatStateOf(0.5f)
    var clean by mutableFloatStateOf(0.2f)
    var ink by mutableIntStateOf(-1)
    /** Where the background was pointed at, or null for the sky behind. */
    var back by mutableStateOf<Offset?>(null)
    var picture by mutableStateOf<ImageBitmap?>(null)
    var busy by mutableStateOf(false)

    fun set() = FineCut(detail, clean, ink, back?.x ?: -1f, back?.y ?: -1f)
}

private val INKS = listOf(-1, 0x000000, 0xFFFFFF, 0x3B2A20, 0x1F3A5F, 0x7A1F2B, 0x1F5F3A)

/**
 * Fine cutout: branches, wires and other fine things cut from the sky or whatever is behind them, down
 * to the twigs, shown on the picture as the sliders move. A tap on the picture says what the background is.
 */
@Composable
fun FineCutPanel(vm: EditorViewModel) {
    val clip = vm.selectedClip ?: run { vm.panel = null; return }
    if (clip.kind != "image") {
        vm.panel = null
        vm.message = "This works on photos. Freeze a frame of the video first"
        return
    }
    val scope = rememberCoroutineScope()
    var applying by remember { mutableStateOf(false) }
    DisposableEffect(clip.id) {
        Fine.back = null
        Fine.ink = -1
        onDispose {
            Fine.picture = null
            Fine.busy = false
            runCatching { releaseFine() }
        }
    }
    LaunchedEffect(clip.path) {
        Fine.picture = null
        snapshotFlow { Fine.set() }.collectLatest { set ->
            val s = vm.session ?: return@collectLatest
            Fine.busy = true
            // the newest setting wins: the ones a drag passed through are dropped
            kotlinx.coroutines.delay(60)
            val made = withContext(Threads.thumbnails) { runCatching { s.finePreview(clip.id, set, 1100u).intoBitmap(null)?.asImageBitmap() } }
            made.onSuccess { Fine.picture = it }.onFailure { vm.message = it.userMessage }
            Fine.busy = false
        }
    }
    PanelFrame("Fine cutout", vm, height = 330.dp, left = {
        if (Fine.back != null) {
            Text("Use sky", style = Type.Small.copy(color = Palette.DockText), modifier = Modifier.clickable { Fine.back = null }.padding(8.dp))
        }
    }) {
        ValueSlider(
            value = Fine.detail * 100f, range = 0f..100f, label = { "${it.roundToInt()}" },
            onChange = { Fine.detail = it / 100f }, onDone = {}, title = "Detail",
        )
        ValueSlider(
            value = Fine.clean * 100f, range = 0f..100f, label = { "${it.roundToInt()}" },
            onChange = { Fine.clean = it / 100f }, onDone = {}, title = "Clean up",
        )
        Row(
            Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            INKS.forEach { ink ->
                val on = Fine.ink == ink
                if (ink < 0) {
                    Box(
                        Modifier.height(30.dp).clip(RoundedCornerShape(15.dp)).background(if (on) Palette.Accent else Color(0xFF3A3A3A)).clickable { Fine.ink = ink }.padding(horizontal = 12.dp),
                        contentAlignment = Alignment.Center,
                    ) { Text("Own color", style = Type.Small.copy(fontSize = 13.sp, color = if (on) Palette.OnAccent else Palette.Text)) }
                } else {
                    Box(
                        Modifier.size(30.dp).clip(CircleShape).background(Color(0xFF000000.toInt() or ink))
                            .border(2.dp, if (on) Palette.Accent else Color(0xFF555555), CircleShape).clickable { Fine.ink = ink },
                    )
                }
                Spacer(Modifier.width(10.dp))
            }
        }
        Text(
            if (Fine.back == null) "The sky is taken out. Tap the photo to take out another background." else "Taking out the color you tapped.",
            style = Type.Small.copy(color = Palette.TextSecondary),
            modifier = Modifier.padding(horizontal = 18.dp),
        )
        Spacer(Modifier.weight(1f))
        Box(Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
            WideButton(if (applying) "Cutting" else "Cut out", !applying && Fine.picture != null) {
                applying = true
                val set = Fine.set()
                vm.edit(settled = { applying = false }) {
                    fineCut(clip.id, set).also { vm.panel = null }
                }
            }
        }
    }
}

/** The photo under Fine cutout: what would be left of it on squares, and a tap to point at the background. */
@Composable
fun FineCutSurface(vm: EditorViewModel, modifier: Modifier) {
    val clip = vm.selectedClip ?: return
    val session = vm.session
    val playhead = vm.playhead
    val box by produceState<ClipBox?>(null, session, vm.timeline?.revision, playhead, clip.id) {
        val s = session ?: return@produceState
        value = withContext(Threads.commands) { runCatching { s.clipBoxes(playhead) }.getOrDefault(emptyList()) }.firstOrNull { it.clipId == clip.id }
    }
    val shape = remember(clip.json) { clipShape(vm, clip) }
    val flips = remember(clip.json) { vm.clipJson(clip.id).let { (it?.optBoolean("flipH") ?: false) to (it?.optBoolean("flipV") ?: false) } }
    val latest by rememberUpdatedState(Triple(box, shape, flips))
    Canvas(
        modifier.pointerInput(clip.id) {
            detectTapGestures { p ->
                val (b, sh, f) = latest
                OnScreen(b ?: return@detectTapGestures, sh, f.first, f.second, size.width.toFloat(), size.height.toFloat()).source(p)?.let { Fine.back = it }
            }
        },
    ) {
        val b = box ?: return@Canvas
        val picture = Fine.picture ?: return@Canvas
        val place = OnScreen(b, shape, flips.first, flips.second, size.width, size.height)
        withTransform({
            translate(place.centre.x, place.centre.y)
            rotate(b.rotation.toFloat(), Offset.Zero)
        }) {
            val half = Offset(place.bw / 2, place.bh / 2)
            withTransform({ clipRect(-half.x, -half.y, half.x, half.y) }) {
                drawRect(Color.White, -half, Size(place.bw, place.bh))
                // the squares a see-through picture is shown on
                val step = 10.dp.toPx()
                var row = 0
                var y = -half.y
                while (y < half.y) {
                    var x = -half.x + if (row % 2 == 0) 0f else step
                    while (x < half.x) {
                        drawRect(Color(0xFFD5D5D5), Offset(x, y), Size(step, step))
                        x += step * 2
                    }
                    y += step
                    row++
                }
                val a = place.local(0f, 0f)
                val c = place.local(1f, 1f)
                withTransform({
                    translate(a.x, a.y)
                    scale(if (c.x < a.x) -1f else 1f, if (c.y < a.y) -1f else 1f, Offset.Zero)
                }) {
                    drawImage(
                        picture,
                        dstOffset = IntOffset.Zero,
                        dstSize = IntSize(abs(c.x - a.x).roundToInt().coerceAtLeast(1), abs(c.y - a.y).roundToInt().coerceAtLeast(1)),
                    )
                }
            }
        }
    }
}
