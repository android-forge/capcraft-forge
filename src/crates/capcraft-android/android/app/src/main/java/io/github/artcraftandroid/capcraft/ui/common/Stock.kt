package io.github.artcraftandroid.capcraft.ui.common

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.util.LruCache
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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.itemsIndexed
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.artcraftandroid.capcraft.ui.Glyph
import io.github.artcraftandroid.capcraft.ui.LucideIcons
import io.github.artcraftandroid.capcraft.ui.Palette
import io.github.artcraftandroid.capcraft.ui.Type
import java.io.File
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject

/** A photo found on a stock site: where its small and full pictures are, whose it is, and what it asks for. */
internal class StockPhoto(
    val id: String,
    val small: String,
    val full: String,
    val width: Int,
    val height: Int,
    val by: String,
    val licence: String,
    val page: String,
    /** A credit is owed wherever it is shown. */
    val credit: Boolean,
    /** Told when the photo is taken, as its site asks. */
    val told: String? = null,
)

/** A site photos come from. One with a [keyPage] searches with a key the user makes there, free, and gives here. */
internal enum class StockSite(val title: String, val keyPage: String? = null, val about: String) {
    COMMONS_FREE("Commons", null, "Wikimedia Commons: public domain and CC0. No credit needed."),
    COMMONS_CREDIT("Commons, with credit", null, "Wikimedia Commons: CC BY. Credit the maker wherever it is shown."),
    NASA("NASA", null, "NASA's own pictures. Free to use; do not suggest NASA endorses anything."),
    PEXELS("Pexels", "https://www.pexels.com/api/", "Photos provided by Pexels."),
    UNSPLASH("Unsplash", "https://unsplash.com/developers", "Photos provided by Unsplash."),
}

/** Photos from stock sites, searched from the import page. Only those at least full HD are shown. */
internal object Stock {
    /** Names the app, and nobody who uses it. */
    private const val AGENT = "CapCraftForge/1.0 (https://github.com/artcraft-android/capcraft-android; Android video editor)"
    private const val FILE = "capcraft-stock"

    private fun prefs(context: Context) = context.getSharedPreferences(FILE, Context.MODE_PRIVATE)

    fun key(context: Context, site: StockSite): String? = prefs(context).getString(site.name, null)?.takeIf { it.isNotBlank() }

    fun setKey(context: Context, site: StockSite, key: String?) = prefs(context).edit().putString(site.name, key?.trim()).apply()

    private fun open(url: String, headers: Map<String, String> = emptyMap()): HttpURLConnection {
        val c = URL(url).openConnection() as HttpURLConnection
        c.connectTimeout = 15_000
        c.readTimeout = 30_000
        c.setRequestProperty("User-Agent", AGENT)
        headers.forEach { (k, v) -> c.setRequestProperty(k, v) }
        return c
    }

    private fun json(url: String, headers: Map<String, String> = emptyMap()): JSONObject {
        val c = open(url, headers)
        try {
            when (val code = c.responseCode) {
                200 -> return JSONObject(c.inputStream.bufferedReader().use { it.readText() })
                401, 403 -> throw IOException("The site did not take that key.")
                429 -> throw IOException("The site asks for a pause. Try again in a while.")
                else -> throw IOException("The site answered $code.")
            }
        } finally {
            c.disconnect()
        }
    }

    private fun q(text: String) = URLEncoder.encode(text, "UTF-8")

    private fun hd(w: Int, h: Int) = maxOf(w, h) >= 1920 && minOf(w, h) >= 1080

    private fun plain(html: String) = html.replace(Regex("<[^>]+>"), "").replace("&amp;", "&").replace("&quot;", "\"").replace("&#039;", "'").replace("&nbsp;", " ").trim()

    /** Page [page] (from 1) of what [query] finds on [site]. */
    fun search(context: Context, site: StockSite, query: String, page: Int): List<StockPhoto> = when (site) {
        StockSite.COMMONS_FREE -> commons(query, page, "unrestricted")
        StockSite.COMMONS_CREDIT -> commons(query, page, "attribution")
        StockSite.NASA -> nasa(query, page)
        StockSite.PEXELS -> pexels(key(context, site).orEmpty(), query, page)
        StockSite.UNSPLASH -> unsplash(key(context, site).orEmpty(), query, page)
    }

    private fun commons(query: String, page: Int, terms: String): List<StockPhoto> {
        val asked = q("$query filemime:image/jpeg filew:>1919 fileh:>1079 haslicense:$terms")
        val found = json(
            "https://commons.wikimedia.org/w/api.php?action=query&format=json&generator=search&gsrnamespace=6&gsrlimit=40&gsroffset=${(page - 1) * 40}" +
                "&gsrsearch=$asked&prop=imageinfo&iiprop=url%7Csize%7Cextmetadata&iiextmetadatafilter=LicenseShortName%7CArtist%7CAttributionRequired&iiurlwidth=330",
        )
        val pages = found.optJSONObject("query")?.optJSONObject("pages") ?: return emptyList()
        return pages.keys().asSequence().map { pages.getJSONObject(it) }.sortedBy { it.optInt("index") }.mapNotNull { p ->
            val i = p.optJSONArray("imageinfo")?.optJSONObject(0) ?: return@mapNotNull null
            val w = i.optInt("width")
            val h = i.optInt("height")
            val small = i.optString("thumburl")
            if (!hd(w, h) || !small.contains("/330px-")) return@mapNotNull null
            val meta = i.optJSONObject("extmetadata")
            fun of(name: String) = plain(meta?.optJSONObject(name)?.optString("value").orEmpty())
            val licence = of("LicenseShortName")
            // a licence that binds what it is used in, or forbids changing it, is left out
            if (Regex("SA|ND|NC|GFDL|GPL", RegexOption.IGNORE_CASE).containsMatchIn(licence.replace("CC0", ""))) return@mapNotNull null
            StockPhoto(
                id = "commons-" + p.optLong("pageid"), small = small,
                // no larger than a 4K frame has use for
                full = if (w > 3840) small.replace("/330px-", "/3840px-") else i.optString("url"),
                width = w, height = h, by = of("Artist").ifBlank { "Unknown" }.take(80), licence = licence,
                page = i.optString("descriptionurl"), credit = terms == "attribution",
            )
        }.toList()
    }

    private fun nasa(query: String, page: Int): List<StockPhoto> {
        val items = json("https://images-api.nasa.gov/search?q=${q(query)}&media_type=image&page_size=60&page=$page").optJSONObject("collection")?.optJSONArray("items") ?: return emptyList()
        return (0 until items.length()).mapNotNull { n ->
            val item = items.getJSONObject(n)
            val data = item.optJSONArray("data")?.optJSONObject(0) ?: return@mapNotNull null
            val links = item.optJSONArray("links") ?: return@mapNotNull null
            val all = (0 until links.length()).map { links.getJSONObject(it) }
            val full = all.firstOrNull { it.optString("rel") == "canonical" } ?: return@mapNotNull null
            val w = full.optInt("width")
            val h = full.optInt("height")
            if (!hd(w, h) || !full.optString("href").lowercase().endsWith(".jpg")) return@mapNotNull null
            val small = all.firstOrNull { it.optString("href").contains("~small") } ?: all.firstOrNull { it.optString("href").contains("~thumb") } ?: return@mapNotNull null
            fun https(u: String) = u.replace("http://", "https://").replace(" ", "%20")
            val maker = data.optString("secondary_creator").ifBlank { data.optString("photographer") }.ifBlank { "NASA" }
            // some are kept there for others, who keep their rights in them
            if (data.optString("description").contains(Regex("copyright|©", RegexOption.IGNORE_CASE))) return@mapNotNull null
            StockPhoto("nasa-" + data.optString("nasa_id"), https(small.optString("href")), https(full.optString("href")), w, h, maker.take(80), "NASA", "https://images.nasa.gov/details/" + Uri.encode(data.optString("nasa_id")), false)
        }
    }

    private fun pexels(key: String, query: String, page: Int): List<StockPhoto> {
        val list = json("https://api.pexels.com/v1/search?query=${q(query)}&per_page=40&page=$page", mapOf("Authorization" to key)).optJSONArray("photos") ?: return emptyList()
        return (0 until list.length()).map { list.getJSONObject(it) }.mapNotNull { p ->
            val w = p.optInt("width")
            val h = p.optInt("height")
            val src = p.optJSONObject("src") ?: return@mapNotNull null
            if (!hd(w, h)) null else StockPhoto("pexels-" + p.optLong("id"), src.optString("medium"), src.optString("original") + "?auto=compress&cs=srgb&fit=max&w=3840&h=3840", w, h, p.optString("photographer"), "Pexels", p.optString("url"), false)
        }
    }

    private fun unsplash(key: String, query: String, page: Int): List<StockPhoto> {
        val list = json("https://api.unsplash.com/search/photos?query=${q(query)}&per_page=30&page=$page", mapOf("Authorization" to "Client-ID $key", "Accept-Version" to "v1")).optJSONArray("results") ?: return emptyList()
        return (0 until list.length()).map { list.getJSONObject(it) }.mapNotNull { p ->
            val w = p.optInt("width")
            val h = p.optInt("height")
            val urls = p.optJSONObject("urls") ?: return@mapNotNull null
            val by = p.optJSONObject("user")?.optString("name").orEmpty()
            if (!hd(w, h)) null else StockPhoto("unsplash-" + p.optString("id"), urls.optString("small"), urls.optString("raw") + "&fm=jpg&q=88&fit=max&w=3840&h=3840", w, h, by, "Unsplash", p.optJSONObject("links")?.optString("html").orEmpty(), false, p.optJSONObject("links")?.optString("download_location"))
        }
    }

    private val small = object : LruCache<String, Bitmap>(24 * 1024 * 1024) {
        override fun sizeOf(key: String, value: Bitmap) = value.byteCount
    }

    fun kept(photo: StockPhoto): Bitmap? = small.get(photo.id)

    fun thumb(photo: StockPhoto): Bitmap? = small.get(photo.id) ?: runCatching {
        val c = open(photo.small)
        try {
            c.inputStream.use { BitmapFactory.decodeStream(it) }
        } finally {
            c.disconnect()
        }
    }.getOrNull()?.also { small.put(photo.id, it) }

    /** Brings [photo] to the phone and returns the file, or throws saying why not. */
    fun fetch(context: Context, site: StockSite, photo: StockPhoto, progress: (Float) -> Unit): File {
        val dir = File(context.cacheDir, "stock").apply { mkdirs() }
        // what was brought in before was copied into its project then
        dir.listFiles()?.filter { it.lastModified() < System.currentTimeMillis() - 24 * 3_600_000L }?.forEach { it.delete() }
        val file = File(dir, photo.id.replace(Regex("[^A-Za-z0-9_-]"), "_") + ".jpg")
        if (file.length() > 0) return file
        val part = File(dir, file.name + ".part")
        // a site asking for a pause is given one, twice at the most
        var c = open(photo.full)
        for (wait in listOf(3L, 8L)) {
            if (c.responseCode != 429 && c.responseCode != 503) break
            val asked = c.getHeaderField("Retry-After")?.toLongOrNull()?.coerceIn(1, 15) ?: wait
            c.disconnect()
            Thread.sleep(asked * 1000)
            c = open(photo.full)
        }
        try {
            if (c.responseCode == 429) throw IOException("The site is busy. Try that photo again in a minute.")
            if (c.responseCode != 200) throw IOException("That photo could not be fetched (${c.responseCode}).")
            val total = c.contentLengthLong
            if (total > 80L * 1024 * 1024) throw IOException("That photo is too large.")
            c.inputStream.use { input ->
                part.outputStream().use { out ->
                    val piece = ByteArray(64 * 1024)
                    var done = 0L
                    while (true) {
                        val n = input.read(piece)
                        if (n < 0) break
                        out.write(piece, 0, n)
                        done += n
                        if (done > 80L * 1024 * 1024) throw IOException("That photo is too large.")
                        if (total > 0) progress(done.toFloat() / total)
                    }
                }
            }
        } catch (e: Throwable) {
            part.delete()
            throw e
        } finally {
            c.disconnect()
        }
        val size = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(part.path, size)
        if (size.outWidth <= 0) {
            part.delete()
            throw IOException("What the site sent is not a photo.")
        }
        part.renameTo(file)
        photo.told?.let { url -> runCatching { open(url, mapOf("Authorization" to "Client-ID ${key(context, site)}")).apply { responseCode; disconnect() } } }
        return file
    }

    fun creditLine(p: StockPhoto) = "Photo: ${p.by}, ${p.licence}, ${p.page}"
}

/** The Stock tab of the import page: a search over the sites, and what it finds, full HD or better. */
@Composable
internal fun StockPage(multiple: Boolean, onDone: (List<Uri>) -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val keyboard = LocalSoftwareKeyboardController.current
    var site by remember { mutableStateOf(StockSite.COMMONS_FREE) }
    var words by remember { mutableStateOf("") }
    var asked by remember { mutableStateOf("") }
    var keys by remember { mutableIntStateOf(0) }
    val found = remember { mutableStateListOf<StockPhoto>() }
    var page by remember { mutableIntStateOf(1) }
    var busy by remember { mutableStateOf(false) }
    var last by remember { mutableStateOf(false) }
    var problem by remember { mutableStateOf<String?>(null) }
    val chosen = remember { mutableStateListOf<StockPhoto>() }
    var fetching by remember { mutableStateOf<String?>(null) }
    val needsKey = site.keyPage != null && remember(site, keys) { Stock.key(context, site) } == null

    LaunchedEffect(site, asked, page, keys) {
        if (asked.isBlank() || needsKey) return@LaunchedEffect
        busy = true
        problem = null
        val result = withContext(Dispatchers.IO) { runCatching { Stock.search(context, site, asked, page) } }
        busy = false
        result.onSuccess { more ->
            val have = found.mapTo(HashSet()) { it.id }
            val fresh = more.filter { have.add(it.id) }
            found.addAll(fresh)
            last = more.isEmpty()
            // a page whose photos were all too small: the next one may hold some
            if (fresh.isEmpty() && more.isNotEmpty() && page < 6) page++
        }.onFailure {
            // a key the site refuses is asked for again
            if (site.keyPage != null && it.message.orEmpty().contains("did not take that key")) {
                Stock.setKey(context, site, null)
                keys++
            }
            problem = it.message?.takeIf { m -> m.isNotBlank() && it !is java.net.UnknownHostException } ?: "There is no internet."
        }
    }
    fun start() {
        keyboard?.hide()
        found.clear()
        chosen.clear()
        last = false
        page = 1
        asked = words.trim()
    }
    fun take(photos: List<StockPhoto>) {
        if (fetching != null || photos.isEmpty()) return
        scope.launch {
            val out = ArrayList<Uri>()
            for ((i, p) in photos.withIndex()) {
                fetching = if (photos.size == 1) "0%" else "${i + 1} of ${photos.size}"
                val r = withContext(Dispatchers.IO) { runCatching { Stock.fetch(context, site, p) { f -> if (photos.size == 1) fetching = "${(f * 100).toInt()}%" } } }
                r.onSuccess { out.add(Uri.fromFile(it)) }.onFailure { problem = it.message ?: "A photo could not be fetched." }
            }
            fetching = null
            if (out.isNotEmpty()) onDone(out)
        }
    }

    Column(Modifier.fillMaxSize()) {
        Row(
            Modifier.padding(horizontal = 12.dp, vertical = 8.dp).fillMaxWidth().height(40.dp).clip(RoundedCornerShape(8.dp)).background(Color(0xFF242424)).padding(horizontal = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Glyph(LucideIcons.Search, Palette.TextSecondary, size = 16.dp)
            Spacer(Modifier.width(8.dp))
            Box(Modifier.weight(1f), contentAlignment = Alignment.CenterStart) {
                if (words.isEmpty()) Text("Search HD photos", style = Type.Body.copy(color = Palette.TextSecondary))
                BasicTextField(
                    words, { words = it.take(100) }, singleLine = true, textStyle = Type.Body.copy(color = Color.White), cursorBrush = SolidColor(Palette.Accent),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search), keyboardActions = KeyboardActions(onSearch = { start() }), modifier = Modifier.fillMaxWidth(),
                )
            }
            if (words.isNotEmpty()) Text("Search", style = Type.BodyStrong.copy(color = Palette.Accent, fontSize = 14.sp), modifier = Modifier.clickable { start() }.padding(8.dp))
        }
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            StockSite.entries.forEach { s ->
                Box(
                    Modifier.height(32.dp).clip(RoundedCornerShape(16.dp)).background(if (s == site) Palette.Accent else Color(0xFF2A2A2A)).clickable(enabled = fetching == null) {
                        if (s != site) {
                            site = s
                            start()
                        }
                    }.padding(horizontal = 13.dp),
                    contentAlignment = Alignment.Center,
                ) { Text(s.title, style = Type.Small.copy(fontSize = 13.sp, color = if (s == site) Color.White else Palette.TextSecondary)) }
            }
        }
        Text(site.about, style = Type.Tiny, modifier = Modifier.padding(horizontal = 14.dp, vertical = 6.dp))
        if (needsKey) {
            KeyForm(site) { key -> Stock.setKey(context, site, key); keys++ }
            return@Column
        }
        Box(Modifier.fillMaxWidth().weight(1f)) {
            val grid = rememberLazyGridState()
            LaunchedEffect(grid) {
                snapshotFlow { grid.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0 }.collect { seen ->
                    if (found.isNotEmpty() && seen >= found.size - 6 && !busy && !last && problem == null && page < 40) page++
                }
            }
            LazyVerticalGrid(
                GridCells.Adaptive(110.dp), Modifier.fillMaxSize(), state = grid, contentPadding = PaddingValues(2.dp),
                verticalArrangement = Arrangement.spacedBy(2.dp), horizontalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                itemsIndexed(found, key = { _, p -> p.id }) { _, p ->
                    val place = chosen.indexOf(p)
                    val picture by produceState(Stock.kept(p), p.id) { if (value == null) value = withContext(Dispatchers.IO) { Stock.thumb(p) } }
                    Box(
                        Modifier.aspectRatio(1f).background(Color(0xFF1F1F1F)).clickable(enabled = fetching == null) {
                            when {
                                !multiple -> take(listOf(p))
                                place >= 0 -> chosen.remove(p)
                                chosen.size < 20 -> chosen.add(p)
                            }
                        },
                    ) {
                        picture?.let { Image(it.asImageBitmap(), contentDescription = p.by, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize()) }
                        if (place >= 0) Box(Modifier.fillMaxSize().background(Color(0x7A06343A)))
                        Text(
                            p.by, style = Type.Tiny.copy(color = Color.White, fontSize = 9.sp), maxLines = 1, overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.align(Alignment.BottomStart).fillMaxWidth().background(Color(0x66000000)).padding(horizontal = 4.dp, vertical = 2.dp),
                        )
                        Text(
                            if (maxOf(p.width, p.height) >= 3840) "4K" else "HD", style = Type.Tiny.copy(color = Color.White, fontSize = 9.sp, fontWeight = FontWeight.SemiBold),
                            modifier = Modifier.align(Alignment.TopStart).padding(4.dp).clip(RoundedCornerShape(3.dp)).background(Color(0x99000000)).padding(horizontal = 4.dp, vertical = 1.dp),
                        )
                        if (multiple) Box(
                            Modifier.align(Alignment.TopEnd).padding(6.dp).size(20.dp).clip(CircleShape)
                                .then(if (place >= 0) Modifier.background(Palette.Accent) else Modifier.border(1.5.dp, Color.White, CircleShape)),
                            contentAlignment = Alignment.Center,
                        ) { if (place >= 0) Text("${place + 1}", style = Type.Small.copy(color = Color.White, fontSize = 11.sp)) }
                    }
                }
            }
            when {
                busy && found.isEmpty() -> CircularProgressIndicator(color = Palette.Accent, strokeWidth = 2.dp, modifier = Modifier.align(Alignment.Center).size(28.dp))
                problem != null && found.isEmpty() -> Text(problem!!, style = Type.Small, textAlign = TextAlign.Center, modifier = Modifier.align(Alignment.Center).padding(24.dp))
                asked.isBlank() -> Text("Type what to look for. The photos come from the site you choose, and need the internet.", style = Type.Small, textAlign = TextAlign.Center, modifier = Modifier.align(Alignment.Center).padding(32.dp))
                found.isEmpty() && !busy -> Text("No HD photos found", style = Type.Small, modifier = Modifier.align(Alignment.Center))
            }
            if (fetching != null) Box(Modifier.fillMaxSize().background(Color(0xAA000000)), contentAlignment = Alignment.Center) { Text("Fetching ${fetching}", style = Type.Body.copy(color = Color.White)) }
        }
        if (multiple) {
            Row(Modifier.fillMaxWidth().background(Color(0xFF1A1A1A)).height(58.dp).padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                val owed = chosen.filter { it.credit || it.licence == "Unsplash" || it.licence == "Pexels" }
                if (owed.isNotEmpty()) {
                    Text(
                        if (owed.any { it.credit }) "Copy credits (needed)" else "Copy credits",
                        style = Type.Small.copy(color = Palette.Accent),
                        modifier = Modifier.clickable {
                            (context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager).setPrimaryClip(ClipData.newPlainText("Credits", owed.joinToString("\n") { Stock.creditLine(it) }))
                        }.padding(vertical = 8.dp),
                    )
                } else if (problem != null && found.isNotEmpty()) Text(problem!!, style = Type.Tiny, modifier = Modifier.weight(1f))
                Spacer(Modifier.weight(1f))
                val ready = chosen.isNotEmpty() && fetching == null
                Box(
                    Modifier.clip(RoundedCornerShape(8.dp)).background(if (ready) Palette.Accent else Color(0xFF1C5A60)).clickable(enabled = ready) { take(chosen.toList()) }.padding(horizontal = 18.dp, vertical = 9.dp),
                ) { Text(if (chosen.isEmpty()) "Add" else "Add (${chosen.size})", style = Type.BodyStrong.copy(color = if (ready) Color.White else Color(0xFF7FA9AD), fontSize = 14.sp)) }
            }
        }
    }
}

@Composable
private fun KeyForm(site: StockSite, onKey: (String) -> Unit) {
    val context = LocalContext.current
    var key by remember(site) { mutableStateOf("") }
    Column(Modifier.fillMaxWidth().padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        Text(
            "${site.title} asks each person for a key of their own. It is free: make one on their site, then paste it here. It stays on this phone.",
            style = Type.Body.copy(color = Palette.Text, textAlign = TextAlign.Center),
        )
        Spacer(Modifier.height(14.dp))
        Text(
            "Open ${site.title}",
            style = Type.BodyStrong.copy(color = Palette.Accent),
            modifier = Modifier.clickable { runCatching { context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(site.keyPage))) } }.padding(10.dp),
        )
        Spacer(Modifier.height(8.dp))
        Box(Modifier.fillMaxWidth().height(44.dp).clip(RoundedCornerShape(8.dp)).background(Color(0xFF242424)).padding(horizontal = 12.dp), contentAlignment = Alignment.CenterStart) {
            if (key.isEmpty()) Text("Your key", style = Type.Body.copy(color = Palette.TextSecondary))
            BasicTextField(key, { key = it.take(200) }, singleLine = true, textStyle = Type.Body.copy(color = Color.White), cursorBrush = SolidColor(Palette.Accent), modifier = Modifier.fillMaxWidth())
        }
        Spacer(Modifier.height(12.dp))
        Box(
            Modifier.fillMaxWidth().height(44.dp).clip(RoundedCornerShape(8.dp)).background(if (key.length > 10) Palette.Accent else Color(0xFF3A3A3A)).clickable(enabled = key.length > 10) { onKey(key) },
            contentAlignment = Alignment.Center,
        ) { Text("Use this key", style = Type.BodyStrong.copy(color = Color.White)) }
    }
}
