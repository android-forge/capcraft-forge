package io.github.artcraftandroid.capcraft.ui.editor.panels

import android.content.Context
import android.content.Intent
import android.media.MediaMetadataRetriever
import android.media.MediaPlayer
import android.net.Uri
import android.provider.DocumentsContract
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.artcraftandroid.capcraft.engine.Media
import io.github.artcraftandroid.capcraft.engine.Threads
import io.github.artcraftandroid.capcraft.ui.Glyph
import io.github.artcraftandroid.capcraft.ui.LucideIcons
import io.github.artcraftandroid.capcraft.ui.Palette
import io.github.artcraftandroid.capcraft.ui.Type
import io.github.artcraftandroid.capcraft.ui.editor.EditorViewModel
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject

/** A sound of the user's own, in a folder they chose: where it is, a readable name, and its folder's. */
internal class FolderSound(val uri: Uri, val name: String, val category: String)

private val AUDIO = setOf("mp3", "wav", "flac", "m4a", "aac", "ogg", "opus")

/** Where the system's folder picker starts: the phone's own [folder]. */
internal fun shelf(folder: String): Uri = DocumentsContract.buildDocumentUri("com.android.externalstorage.documents", "primary:$folder")

/** More than this many sounds in the folders are left out: the list is kept in memory. */
private const val MOST_SOUNDS = 5000

/**
 * A folder's name as a category: "BUTTON", "CLICK" and "Click" read alike, "Computer _ Robot" is
 * "Computer / Robot", and a library's initials ("ES Electric") go.
 */
internal fun soundCategory(folder: String): String {
    val words = folder.replace(" _ ", " / ").replace('_', ' ').replace(Regex("\\s*\\(\\d+\\)$"), "").removePrefix("ES ").trim()
    // a word all in capitals is a word, unless it is as short as initials are ("3D", "UI")
    return words.split(' ').joinToString(" ") { w -> if (w.length > 2 && w == w.uppercase()) w.lowercase().replaceFirstChar { it.uppercase() } else w.replaceFirstChar { it.uppercase() } }
        .ifEmpty { "Sounds" }
}

/**
 * A sound file's name to read: "wolf-howl-one-shot_zJq_xSNO.mp3" is "Wolf howl one shot" and
 * "lion-roar-2-SBA-300098917.mp3" is "Lion roar 2"; one only numbered ("sound-3", "Error 00012") takes its [category]'s.
 */
internal fun soundName(file: String, category: String): String {
    var s = file.substringBeforeLast('.')
    s = s.replace(Regex("\\s*\\(\\d+\\)$"), "")
        .replace(Regex("[-_ ]SBA[-_ ]\\d+.*$"), "")
        .replace(Regex("^ES_"), "").replace(Regex("\\s*-\\s*SFX Producer.*$"), "")
        .replace(Regex("\\s*\\((MP3|HD|WAV|Copyright Free|Lossless)[^)]*\\)", RegexOption.IGNORE_CASE), "")
        .replace(Regex("\\s*-?\\s*\\bsound effects?\\b", RegexOption.IGNORE_CASE), "")
    // a download's random tail: an underscore and eight letters and digits, capitals in the middle of them as no word has
    s = s.trim('_', ' ').replace(Regex("_(?=[A-Za-z0-9_]{8}$)(?=.*([a-z0-9][A-Z]|[A-Z]{2}))[A-Za-z0-9_]{8}$"), "")
    s = s.replace('-', ' ').replace('_', ' ').replace('.', ' ').replace(Regex("\\s+"), " ").trim()
    val only = Regex("^(?:sound ?)?0*(\\d+)?$", RegexOption.IGNORE_CASE).matchEntire(s)
    if (only != null) s = (category + " " + only.groupValues[1]).trim()
    // zeroes before a number are a sort order, not a name
    s = s.replace(Regex("\\b0+(\\d+)$"), "$1")
    return s.replaceFirstChar { it.uppercase() }.ifEmpty { category }
}

/** The folders of sounds the user chose, and what was found in them: their own files, read where they are. */
internal object SoundFolders {
    private const val FILE = "capcraft-sounds"
    private const val KEY = "folders"

    private fun prefs(context: Context) = context.getSharedPreferences(FILE, Context.MODE_PRIVATE)

    fun folders(context: Context): List<Uri> {
        val all = runCatching { JSONArray(prefs(context).getString(KEY, "[]")) }.getOrDefault(JSONArray())
        return (0 until all.length()).map { Uri.parse(all.optString(it)) }
    }

    private fun keep(context: Context, list: List<Uri>) =
        prefs(context).edit().putString(KEY, JSONArray(list.map { it.toString() }).toString()).apply()

    fun add(context: Context, tree: Uri) {
        context.contentResolver.takePersistableUriPermission(tree, Intent.FLAG_GRANT_READ_URI_PERMISSION)
        keep(context, (folders(context) + tree).distinct())
        index(context).delete()
    }

    fun clear(context: Context) {
        folders(context).forEach { runCatching { context.contentResolver.releasePersistableUriPermission(it, Intent.FLAG_GRANT_READ_URI_PERMISSION) } }
        keep(context, emptyList())
        index(context).delete()
    }

    private fun index(context: Context) = File(context.filesDir, "sound-folders.json")

    /** What was found last time, or null when the folders are to be read. */
    fun known(context: Context): List<FolderSound>? = runCatching {
        val all = JSONArray(index(context).takeIf { it.exists() }?.readText() ?: return null)
        (0 until all.length()).map { all.getJSONObject(it) }.map { FolderSound(Uri.parse(it.getString("u")), it.getString("n"), it.getString("c")) }
    }.getOrNull()

    /** Reads every folder, a folder in a pack its category; a sound found twice (the same name and size) is listed once. */
    fun read(context: Context): List<FolderSound> {
        val found = ArrayList<FolderSound>()
        val seen = HashSet<String>()
        for (tree in folders(context)) {
            val root = runCatching { DocumentsContract.getTreeDocumentId(tree) }.getOrNull() ?: continue
            // the folder still to read, and the category its sounds go in: the first folder down names it
            val todo = ArrayDeque(listOf(Triple(root, "Sounds", 0)))
            while (todo.isNotEmpty() && found.size < MOST_SOUNDS) {
                val (id, category, depth) = todo.removeFirst()
                val folders = ArrayList<Pair<String, String>>()
                var here = 0
                runCatching {
                    context.contentResolver.query(
                        DocumentsContract.buildChildDocumentsUriUsingTree(tree, id),
                        arrayOf(
                            DocumentsContract.Document.COLUMN_DOCUMENT_ID,
                            DocumentsContract.Document.COLUMN_DISPLAY_NAME,
                            DocumentsContract.Document.COLUMN_MIME_TYPE,
                            DocumentsContract.Document.COLUMN_SIZE,
                        ),
                        null, null, null,
                    )?.use { c ->
                        while (c.moveToNext() && found.size < MOST_SOUNDS) {
                            val name = c.getString(1) ?: continue
                            if (c.getString(2) == DocumentsContract.Document.MIME_TYPE_DIR) {
                                folders += c.getString(0) to name
                            } else if (name.substringAfterLast('.').lowercase() in AUDIO && !name.startsWith(".")) {
                                here++
                                val bare = name.replace(Regex("\\s*\\(\\d+\\)(?=\\.[^.]+$)"), "").lowercase()
                                if (!seen.add(bare + "|" + c.getLong(3))) continue
                                found.add(FolderSound(DocumentsContract.buildDocumentUriUsingTree(tree, c.getString(0)), soundName(name, category), category))
                            }
                        }
                    }
                }
                // a folder of many folders and no sounds is a pack: each folder in it names its own; else they are more of the same
                val pack = depth == 0 || (here == 0 && folders.size >= 8)
                if (depth < 6) folders.forEach { (child, name) -> todo.add(Triple(child, if (pack) soundCategory(name) else category, depth + 1)) }
            }
        }
        // a name two sounds share is told apart by a number
        val counts = HashMap<String, Int>()
        val named = found.sortedWith(compareBy({ it.category.lowercase() }, { it.name.lowercase() })).map { s ->
            val n = (counts[s.category + s.name] ?: 0) + 1
            counts[s.category + s.name] = n
            if (n == 1) s else FolderSound(s.uri, "${s.name} ($n)", s.category)
        }
        runCatching {
            index(context).writeText(JSONArray(named.map { JSONObject().put("u", it.uri.toString()).put("n", it.name).put("c", it.category) }).toString())
        }
        return named
    }
}

/**
 * The user's own sound effects: folders on this phone they chose, listed by folder and searchable.
 * The files stay where they are; one put on the timeline is copied into the project.
 */
@Composable
internal fun FolderSounds(vm: EditorViewModel, query: String, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var reads by remember { mutableIntStateOf(0) }
    var sounds by remember { mutableStateOf(SoundFolders.known(context)) }
    var category by remember { mutableIntStateOf(0) }
    var playing by remember { mutableStateOf<Uri?>(null) }
    val player = remember { MediaPlayer() }
    DisposableEffect(Unit) { onDispose { runCatching { player.release() } } }
    LaunchedEffect(reads) {
        if (sounds == null && SoundFolders.folders(context).isNotEmpty()) sounds = withContext(Dispatchers.IO) { SoundFolders.read(context) }
    }
    val choose = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { tree ->
        if (tree != null) {
            runCatching { SoundFolders.add(context, tree) }
                .onSuccess { sounds = null; category = 0; reads++ }
                .onFailure { vm.message = "Could not read that folder" }
        }
    }

    fun play(s: FolderSound) {
        runCatching {
            player.reset()
            if (playing == s.uri) {
                playing = null
                return
            }
            player.setDataSource(context, s.uri)
            player.setOnPreparedListener { it.start() }
            player.setOnCompletionListener { playing = null }
            player.setOnErrorListener { _, _, _ -> playing = null; true }
            player.prepareAsync()
            playing = s.uri
        }.onFailure { playing = null; vm.message = "Could not play that sound" }
    }

    fun add(s: FolderSound) {
        runCatching { player.reset() }
        playing = null
        scope.launch {
            val file = withContext(Threads.commands) { runCatching { Media.importUri(context, vm.path, s.uri) }.getOrNull() }
            if (file == null) vm.message = "Could not read that sound" else placeFx(vm, file, s.name)
        }
    }

    Column(modifier.fillMaxWidth()) {
        val all = sounds
        when {
            SoundFolders.folders(context).isEmpty() -> Column(Modifier.fillMaxSize().padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                Spacer(Modifier.height(16.dp))
                Glyph(LucideIcons.Folder, Palette.DockText, size = 32.dp)
                Spacer(Modifier.height(12.dp))
                Text(
                    "Choose a folder of sounds on this phone. The folders in it become categories; the files stay where they are.",
                    style = Type.Body.copy(color = Palette.Text, textAlign = TextAlign.Center),
                )
                Spacer(Modifier.height(18.dp))
                Box(
                    Modifier.fillMaxWidth().height(44.dp).clip(RoundedCornerShape(8.dp)).background(Palette.Accent).clickable { choose.launch(shelf("Music")) },
                    contentAlignment = Alignment.Center,
                ) { Text("Choose folder", style = Type.BodyStrong.copy(fontSize = 16.sp, color = Palette.OnAccent)) }
            }
            all == null -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text("Reading your sounds", style = Type.Body.copy(color = Palette.TextSecondary))
            }
            else -> {
                val categories = remember(all) { listOf("All") + all.map { it.category }.distinct() }
                val needle = query.trim()
                val shown = when {
                    needle.isNotEmpty() -> all.filter { it.name.contains(needle, true) || it.category.contains(needle, true) }
                    category == 0 -> all
                    else -> all.filter { it.category == categories.getOrNull(category) }
                }
                Tabs(categories, if (needle.isEmpty()) category else -1) { category = it }
                LazyColumn(Modifier.fillMaxWidth().weight(1f)) {
                    items(shown, key = { it.uri.toString() }) { s ->
                        FolderSoundRow(s, playing == s.uri, showCategory = needle.isNotEmpty() || category == 0, onPlay = { play(s) }, onAdd = { add(s) })
                    }
                    if (shown.isEmpty()) item { Text("No sounds here", style = Type.Small.copy(color = Palette.TextSecondary), modifier = Modifier.padding(20.dp)) }
                    item {
                        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 10.dp)) {
                            FolderAction("Add folder", Modifier.weight(1f)) { choose.launch(shelf("Music")) }
                            Spacer(Modifier.width(8.dp))
                            FolderAction("Read again", Modifier.weight(1f)) {
                                sounds = null
                                scope.launch { sounds = withContext(Dispatchers.IO) { SoundFolders.read(context) } }
                            }
                            Spacer(Modifier.width(8.dp))
                            FolderAction("Forget folders", Modifier.weight(1f)) {
                                SoundFolders.clear(context)
                                sounds = null
                                category = 0
                                reads++
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun FolderAction(label: String, modifier: Modifier, onClick: () -> Unit) {
    Box(
        modifier.height(36.dp).clip(RoundedCornerShape(18.dp)).border(1.dp, Color(0xFF4A4A4A), RoundedCornerShape(18.dp)).clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) { Text(label, style = Type.Small.copy(fontSize = 12.sp, color = Palette.DockText)) }
}

@Composable
private fun FolderSoundRow(s: FolderSound, on: Boolean, showCategory: Boolean, onPlay: () -> Unit, onAdd: () -> Unit) {
    val context = LocalContext.current
    val seconds by produceState<Int?>(null, s.uri) {
        value = withContext(Dispatchers.IO) {
            runCatching {
                val r = MediaMetadataRetriever()
                try {
                    r.setDataSource(context, s.uri)
                    r.extractMetadata(MediaMetadataRetriever.METADATA_KEY_DURATION)?.toLong()?.let { ((it + 999) / 1000).toInt() }
                } finally {
                    r.release()
                }
            }.getOrNull()
        }
    }
    Row(
        Modifier.fillMaxWidth().height(52.dp).clickable(onClick = onPlay).padding(start = 18.dp, end = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Glyph(if (on) LucideIcons.AudioLines else LucideIcons.Music, if (on) Palette.Accent else Palette.TextSecondary, size = 18.dp)
        Spacer(Modifier.width(14.dp))
        Column(Modifier.weight(1f)) {
            Text(s.name, style = Type.Body.copy(fontSize = 15.sp, color = if (on) Palette.Accent else Palette.Text), maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (showCategory) Text(s.category, style = Type.Tiny.copy(color = Palette.TextSecondary), maxLines = 1)
        }
        Spacer(Modifier.width(8.dp))
        seconds?.let { Text("%02d:%02d".format(it / 60, it % 60), style = Type.Small.copy(fontSize = 12.sp, color = Palette.TextSecondary)) }
        Spacer(Modifier.width(14.dp))
        Box(
            Modifier.size(34.dp).clip(CircleShape).background(Color(0xFF3A3A3A)).clickable(onClick = onAdd),
            contentAlignment = Alignment.Center,
        ) { Glyph(LucideIcons.Plus, Palette.Text, size = 18.dp, description = "Add ${s.name}") }
    }
}
