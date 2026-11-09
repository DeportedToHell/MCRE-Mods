// Every sound the client starts passes Bad Wither No Cookie's check
// (`SoundEngineMixin`, before `canPlaySound`): silenced, it doesn't start.
fn bwncr_sound(sound: &crate::sound_engine::SoundPlayback<'_>, cancel: &mut bool) {
    if crate::sound_playback::bwncr_sounds::play(sound.instance, sound.identifier, sound.starting) {
        *cancel = true;
    }
}