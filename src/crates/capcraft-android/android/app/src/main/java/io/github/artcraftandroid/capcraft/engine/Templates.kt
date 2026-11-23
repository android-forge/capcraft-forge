package io.github.artcraftandroid.capcraft.engine

import android.content.Context
import android.net.Uri
import io.github.artcraftandroid.capcraft.core.MobileSession
import io.github.artcraftandroid.capcraft.core.ProjectSummary
import io.github.artcraftandroid.capcraft.core.RowKind
import io.github.artcraftandroid.capcraft.core.duplicateProject

/** A project of the user's own used as a template: its cuts, titles, effects and music with other media in its clips. */
object Templates {
    /**
     * Copies [p] and puts [uris] in its clips' places: the main track's in order, then the videos and
     * pictures over it. Clips past the last of them keep the template's own. Answers the copy's folder.
     */
    fun fill(context: Context, p: ProjectSummary, uris: List<Uri>): String {
        val copy = duplicateProject(p.path, p.name.ifBlank { "Untitled" } + " copy")
        val session = MobileSession.open(copy.path)
        try {
            val rows = session.view().rows
            val reel = session.extra("capcraftReel").isNotEmpty()
            val slots = session.slots().mapTo(HashSet()) { it.clipId }
            val places = rows.filter { it.kind == RowKind.MAIN || it.kind == RowKind.OVERLAY }
                .sortedBy { it.kind != RowKind.MAIN }
                .flatMap { row -> row.clips.sortedBy { it.startUs } }
                .filter { it.kind == "video" || it.kind == "image" }
            places.zip(uris).forEach { (clip, uri) ->
                val file = Media.importUri(context, copy.path, uri)
                // one made from a reel moves what goes in as the reel's own moved
                if (reel && clip.id in slots) session.fillSlot(clip.id, file.absolutePath, true, true) else session.replaceMedia(clip.id, file.absolutePath)
            }
            session.save()
        } finally {
            session.destroy()
        }
        return copy.path
    }
}
