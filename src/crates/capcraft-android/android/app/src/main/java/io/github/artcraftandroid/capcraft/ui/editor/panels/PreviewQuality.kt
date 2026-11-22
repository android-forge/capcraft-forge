package io.github.artcraftandroid.capcraft.ui.editor.panels

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.artcraftandroid.capcraft.ui.Palette
import io.github.artcraftandroid.capcraft.ui.Type
import io.github.artcraftandroid.capcraft.ui.editor.EditorViewModel

private val CHOICES = listOf(Triple("Full", "As sharp as the screen", 1f), Triple("Half", "Faster", 0.5f), Triple("Low", "For slower phones", 0.33f))

/** How sharp the preview is drawn, for phones that fall behind playing it; an export is always whole. */
@Composable
fun PreviewQualityPanel(vm: EditorViewModel) {
    PanelFrame("Preview quality", vm, height = 220.dp) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 18.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            CHOICES.forEach { (name, hint, scale) ->
                val on = kotlin.math.abs(vm.previewScale - scale) < 0.01f
                Column(
                    Modifier.weight(1f).height(74.dp).clip(RoundedCornerShape(8.dp)).background(Color(0xFF333333))
                        .border(1.5.dp, if (on) Palette.Accent else Color.Transparent, RoundedCornerShape(8.dp))
                        .clickable { vm.setPreview(scale) },
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.Center,
                ) {
                    Text(name, style = Type.BodyStrong.copy(fontSize = 15.sp, color = if (on) Palette.Accent else Palette.Text))
                    Spacer(Modifier.height(4.dp))
                    Text(hint, style = Type.Tiny.copy(color = Palette.TextSecondary))
                }
            }
        }
        Text("Exports are not changed.", style = Type.Small, modifier = Modifier.padding(horizontal = 18.dp))
    }
}
