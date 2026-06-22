// AudioWorklet processor: converts mic audio to 16-bit PCM at a target sample
// rate and posts configured-duration chunks to the main thread for streaming
// STT proxy. Lives in /public so it is served verbatim and loaded via
// audioWorklet.addModule('/pcm-processor.js').
class PcmProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();
    const opts = (options && options.processorOptions) || {};
    this.targetRate = opts.targetRate;
    this.chunkDurationMs = opts.chunkDurationMs;
    if (!(this.targetRate > 0) || !(this.chunkDurationMs > 0)) {
      throw new Error('PCM processor requires positive targetRate and chunkDurationMs options');
    }
    this.frameSamples = Math.max(
      1,
      Math.round(this.targetRate * this.chunkDurationMs / 1000),
    );
    this.acc = new Int16Array(this.frameSamples);
    this.accLen = 0;
  }

  process(inputs) {
    const channel = inputs[0] && inputs[0][0];
    if (!channel || channel.length === 0) return true;

    // Decimate from the context rate down to the target rate (linear pick).
    const ratio = sampleRate / this.targetRate;
    const outLen = Math.floor(channel.length / ratio);
    for (let i = 0; i < outLen; i++) {
      let s = channel[Math.floor(i * ratio)];
      s = Math.max(-1, Math.min(1, s));
      this.acc[this.accLen++] = s < 0 ? s * 0x8000 : s * 0x7fff;
      if (this.accLen === this.frameSamples) {
        const buf = this.acc.buffer;
        this.port.postMessage(buf, [buf]);
        this.acc = new Int16Array(this.frameSamples);
        this.accLen = 0;
      }
    }
    return true;
  }
}

registerProcessor('pcm-processor', PcmProcessor);
