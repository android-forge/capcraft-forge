package io.github.artcraftandroid.capcraft.ui.editor.panels

import android.net.Uri
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.itemsIndexed
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.artcraftandroid.capcraft.core.Slot
import io.github.artcraftandroid.capcraft.engine.Media
import io.github.artcraftandroid.capcraft.engine.Threads
import io.github.artcraftandroid.capcraft.engine.userMessage
import io.github.artcraftandroid.capcraft.ui.Glyph
import io.github.artcraftandroid.capcraft.ui.LucideIcons
import io.github.artcraftandroid.capcraft.ui.Palette
import io.github.artcraftandroid.capcraft.ui.Type
import io.github.artcraftandroid.capcraft.ui.common.MediaKinds
import io.github.artcraftandroid.capcraft.ui.common.MediaPick
import io.github.artcraftandroid.capcraft.ui.editor.EditorViewModel
import io.github.artcraftandroid.capcraft.ui.editor.Thumbs
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * Replace media: the places media go in a template, each with how long it lasts. One is filled at a tap,
 * or all of them at once from a pick of several, used again from the first when they run out.
 */
@Composable
fun SlotsPanel(vm: EditorViewModel) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val session = vm.session
    val slots by produceState<List<Slot>>(emptyList(), session, vm.timeline?.revision) {
        value = session?.let { s -> withContext(Threads.commands) { runCatching { s.slots() }.getOrDefault(emptyList()) } }.orEmpty()
    }
    val words by produceState(0, session, vm.timeline?.revision) {
        value = session?.let { s -> withContext(Threads.commands) { runCatching { s.reelWords().toInt() }.getOrDefault(0) } } ?: 0
    }
    var moves by remember { mutableStateOf(true) }
    var filling by remember { mutableStateOf<String?>(null) }

    fun fill(places: List<Slot>, uris: List<Uri>) {
        val s = session ?: return
        if (uris.isEmpty() || places.isEmpty() || filling != null) return
        filling = "0 of ${places.size}"
        scope.launch {
            val result = withContext(Threads.commands) {
                runCatching {
                    // brought in once, however many places it goes
                    val files = HashMap<Uri, String>()
                    var view = s.view()
                    places.forEachIndexed { i, place ->
                        filling = "${i + 1} of ${places.size}"
                        val uri = uris[i % uris.size]
                        val file = files.getOrPut(uri) { Media.importUri(context, vm.path, uri).absolutePath }
                        view = s.fillSlot(place.clipId, file, moves)
                    }
                    view
                }
            }
            filling = null
            result.onSuccess { vm.applyView(it) }.onFailure { vm.message = it.userMessage }
        }
    }

    PanelFrame("Replace media", vm, tall = true, titleOnTop = true) {
        Row(
            Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 12.dp, vertical = 6.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Pick("Fill all", true) { MediaPick.open(MediaKinds.BOTH, multiple = true) { uris -> fill(slots, uris) } }
            val empty = slots.filter { !it.filled }
            if (empty.isNotEmpty() && empty.size < slots.size) Pick("Fill the rest (${empty.size})", false) { MediaPick.open(MediaKinds.BOTH, multiple = true) { uris -> fill(empty, uris) } }
            if (slots.any { it.moves }) Pick("Camera moves", moves) { moves = !moves }
            if (words > 0) Pick(if (words == 1) "Add its title" else "Add its $words titles", false) {
                vm.edit { addReelWords() }
                vm.message = "Titles added where the video had them"
            }
        }
        Text(
            filling?.let { "Filling $it" } ?: "${slots.size} place${if (slots.size == 1) "" else "s"}. Tap one to put a photo or video there.",
            style = Type.Tiny, modifier = Modifier.padding(horizontal = 14.dp, vertical = 2.dp),
        )
        LazyVerticalGrid(
            GridCells.Fixed(4),
            Modifier.fillMaxWidth().weight(1f),
            contentPadding = PaddingValues(12.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            itemsIndexed(slots, key = { _, slot -> slot.clipId }) { i, slot ->
                @Suppress("UNUSED_VARIABLE") val drawn = Thumbs.version.intValue
                val picture = remember(slot.path, slot.sourceUs, drawn) { Thumbs.still(slot.path, slot.sourceUs + slot.durationUs / 3, 240)?.asImageBitmap() }
                Column(
                    Modifier.clickable(enabled = filling == null) {
                        vm.scrub(slot.startUs)
                        MediaPick.open(MediaKinds.BOTH, multiple = false) { uris -> fill(listOf(slot), uris) }
                    },
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Box(
                        Modifier.fillMaxWidth().aspectRatio(0.8f).clip(RoundedCornerShape(8.dp)).background(Color(0xFF333333))
                            .border(if (slot.filled) 2.dp else 0.dp, if (slot.filled) Palette.Accent else Color.Transparent, RoundedCornerShape(8.dp)),
                    ) {
                        picture?.let { Image(it, contentDescription = "Place ${i + 1}", contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize()) }
                        Text(
                            "${i + 1}", style = Type.Tiny.copy(color = Color.White, fontSize = 10.sp),
                            modifier = Modifier.align(Alignment.TopStart).padding(4.dp).clip(CircleShape).background(Color(0x99000000)).padding(horizontal = 6.dp, vertical = 2.dp),
                        )
                        if (slot.moves) Box(Modifier.align(Alignment.TopEnd).padding(4.dp).size(18.dp).clip(CircleShape).background(Color(0x99000000)), contentAlignment = Alignment.Center) {
                            Glyph(LucideIcons.ZoomIn, Color.White, size = 11.dp, description = "Moves")
                        }
                    }
                    Spacer(Modifier.height(4.dp))
                    Text("%.1fs".format(slot.durationUs / 1e6), style = Type.Tiny)
                }
            }
        }
    }
}

@Composable
private fun Pick(label: String, on: Boolean, onClick: () -> Unit) {
    Box(
        Modifier.height(34.dp).clip(RoundedCornerShape(17.dp)).background(if (on) Palette.Accent else Color(0xFF3A3A3A)).clickable(onClick = onClick).padding(horizontal = 14.dp),
        contentAlignment = Alignment.Center,
    ) { Text(label, style = Type.Small.copy(fontSize = 13.sp, color = if (on) Palette.OnAccent else Palette.Text)) }
}
