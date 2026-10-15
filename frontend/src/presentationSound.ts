export type SoundPreset = 'move' | 'capture' | 'drop' | 'ability' | 'explosion' | 'victory' | 'defeat'

const tones: Record<SoundPreset, [number, number, number]> = {
  move: [440, 330, 0.055], capture: [180, 95, 0.13], drop: [520, 760, 0.12],
  ability: [600, 380, 0.14], explosion: [130, 55, 0.2],
  victory: [523, 784, 0.33], defeat: [280, 150, 0.34],
}

class PresentationSound {
  private context: AudioContext | null = null
  private volume = 0.25
  private muted = false

  get isMuted() { return this.muted }
  setVolume(value: number) { this.volume = Number.isFinite(value) ? Math.max(0, Math.min(1, value)) : this.volume }
  setMuted(value: boolean) { this.muted = value }
  play(preset: SoundPreset) {
    if (this.muted || this.volume === 0 || typeof window === 'undefined') return
    try {
      this.context ??= new AudioContext()
      if (this.context.state === 'suspended') void this.context.resume().catch(() => {})
      const [start, end, duration] = tones[preset]
      const oscillator = this.context.createOscillator()
      const gain = this.context.createGain()
      const now = this.context.currentTime
      oscillator.type = preset === 'capture' || preset === 'explosion' ? 'triangle' : 'sine'
      oscillator.frequency.setValueAtTime(start, now)
      oscillator.frequency.exponentialRampToValueAtTime(end, now + duration)
      gain.gain.setValueAtTime(Math.max(0.001, this.volume * 0.4), now)
      gain.gain.exponentialRampToValueAtTime(0.001, now + duration)
      oscillator.connect(gain).connect(this.context.destination)
      oscillator.onended = () => { oscillator.disconnect(); gain.disconnect() }
      oscillator.start(now)
      oscillator.stop(now + duration)
    } catch { /* Audio is optional; gameplay continues when unavailable. */ }
  }
}

export const presentationSound = new PresentationSound()
