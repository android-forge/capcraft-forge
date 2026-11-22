package io.github.artcraftandroid.capcraft.engine

import android.content.Context
import android.net.Uri
import io.github.artcraftandroid.capcraft.core.ProjectSummary
import io.github.artcraftandroid.capcraft.core.adoptProject
import java.io.BufferedInputStream
import java.io.BufferedOutputStream
import java.io.File
import java.util.zip.Deflater
import java.util.zip.ZipEntry
import java.util.zip.ZipInputStream
import java.util.zip.ZipOutputStream
import org.json.JSONObject

/** A project as one file to keep or move to another phone: its folder zipped, and read back as a new project. */
object ProjectZip {
    /** Beside the project's own files in the zip: which folder they were made in, for the paths in the document. */
    private const val NOTE = "capcraft-project.json"

    /** Files in the cache clips are pointed at; the rest of it is made again on use (the bridge's `KEPT_COPIES`). */
    private val KEPT = listOf("reverse-", "enhance-", "stabilize-")

    /** More than this in an imported zip is refused: files, and bytes once unpacked. */
    private const val MOST_FILES = 20_000
    private const val MOST_BYTES = 64L shl 30

    /** Writes the project in [path] to [to] as a zip; [progress] gets how far along, 0 to 1. */
    fun write(context: Context, path: String, to: Uri, progress: (Float) -> Unit) {
        val root = File(path)
        val files = root.walkTopDown().onEnter { it == root || it.name != "cache" }.filter { it.isFile }.toList() +
            File(root, "cache").listFiles().orEmpty().filter { f -> f.isFile && KEPT.any { f.name.startsWith(it) } && ".part." !in f.name }
        val all = files.sumOf { it.length() }.coerceAtLeast(1)
        var done = 0L
        val out = context.contentResolver.openOutputStream(to, "wt") ?: throw IllegalStateException("Could not write the file.")
        ZipOutputStream(BufferedOutputStream(out, 1 shl 16)).use { zip ->
            // videos and pictures are packed already: packing them again only takes time
            zip.setLevel(Deflater.BEST_SPEED)
            zip.putNextEntry(ZipEntry(NOTE))
            zip.write(JSONObject().put("version", 1).put("root", root.absolutePath).toString().toByteArray())
            zip.closeEntry()
            val buffer = ByteArray(1 shl 16)
            files.forEach { f ->
                zip.putNextEntry(ZipEntry("project/" + f.relativeTo(root).invariantSeparatorsPath))
                f.inputStream().use { input ->
                    while (true) {
                        val n = input.read(buffer)
                        if (n < 0) break
                        zip.write(buffer, 0, n)
                        done += n
                        progress(done.toFloat() / all)
                    }
                }
                zip.closeEntry()
            }
        }
    }

    /** Reads a project zip written by [write] into a new project. */
    fun read(context: Context, from: Uri): ProjectSummary {
        val staging = File(context.cacheDir, "import-" + System.nanoTime()).apply { mkdirs() }
        try {
            val home = File(staging, "project").apply { mkdirs() }
            val base = home.canonicalPath + File.separator
            var root: String? = null
            var count = 0
            var bytes = 0L
            val input = context.contentResolver.openInputStream(from) ?: throw IllegalStateException("Could not read the file.")
            ZipInputStream(BufferedInputStream(input, 1 shl 16)).use { zip ->
                val buffer = ByteArray(1 shl 16)
                while (true) {
                    val entry = zip.nextEntry ?: break
                    if (entry.isDirectory) continue
                    if (++count > MOST_FILES) throw IllegalStateException("That file is too big to be a project.")
                    if (entry.name == NOTE) {
                        val note = ByteArray(8192)
                        var n = 0
                        while (n < note.size) {
                            val r = zip.read(note, n, note.size - n)
                            if (r < 0) break
                            n += r
                        }
                        root = JSONObject(String(note, 0, n)).optString("root")
                        continue
                    }
                    if (!entry.name.startsWith("project/")) continue
                    // a name that climbs out of the folder is not written
                    val out = File(home, entry.name.removePrefix("project/"))
                    if (!out.canonicalPath.startsWith(base)) continue
                    out.parentFile?.mkdirs()
                    out.outputStream().use { o ->
                        while (true) {
                            val n = zip.read(buffer)
                            if (n < 0) break
                            bytes += n
                            if (bytes > MOST_BYTES) throw IllegalStateException("That file is too big to be a project.")
                            o.write(buffer, 0, n)
                        }
                    }
                }
            }
            val old = root?.takeIf { it.startsWith("/") && it.length > 8 } ?: throw IllegalStateException("That is not a CapCraft project file.")
            return adoptProject(home.absolutePath, old)
        } finally {
            staging.deleteRecursively()
        }
    }
}
