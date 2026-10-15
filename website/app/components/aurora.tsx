'use client';

import { useEffect, useRef, useState } from 'react';
import { Pause, Play } from 'lucide-react';

// Original Schematlas artwork: domain-warped colour fields with film grain, drawn behind the
// product. Four colours per palette; the CSS gradient underneath is the fallback and the first
// frame, so nothing flashes while WebGL starts or when it is unavailable.
export const palettes = {
  spring: ['#effcf3', '#57ff8b', '#13a874', '#c9f26a'],
  lagoon: ['#eefbf6', '#7cf0b9', '#22a99d', '#9fe3f5'],
  forest: ['#f1fff4', '#57ff8b', '#0e7a53', '#ddfb8d'],
} as const;
export type Palette = keyof typeof palettes;

const vertex = `attribute vec2 a_position; void main(){ gl_Position=vec4(a_position,0.,1.); }`;
const fragment = `
precision highp float;
uniform vec2 u_res;
uniform float u_time;
uniform vec3 u_c0;
uniform vec3 u_c1;
uniform vec3 u_c2;
uniform vec3 u_c3;
float hash(vec2 p){ return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453); }
float noise(vec2 p){
 vec2 i=floor(p), f=fract(p);
 vec2 u=f*f*(3.-2.*f);
 return mix(mix(hash(i),hash(i+vec2(1.,0.)),u.x),mix(hash(i+vec2(0.,1.)),hash(i+vec2(1.,1.)),u.x),u.y);
}
float fbm(vec2 p){
 float v=0., a=.5;
 for(int i=0;i<5;i++){ v+=a*noise(p); p=mat2(1.6,1.2,-1.2,1.6)*p; a*=.5; }
 return v;
}
void main(){
 vec2 p=(gl_FragCoord.xy-.5*u_res)/min(u_res.x,u_res.y);
 float t=u_time*.045;
 vec2 q=vec2(fbm(p*1.3+vec2(0.,t)),fbm(p*1.3+vec2(5.2,-t)));
 vec2 r=vec2(fbm(p*1.1+1.8*q+vec2(1.7,9.2)+t*1.4),fbm(p*1.1+1.8*q+vec2(8.3,2.8)-t));
 float f=fbm(p*.9+2.2*r);
 vec3 col=mix(u_c0,u_c1,smoothstep(.25,.75,f));
 col=mix(col,u_c2,smoothstep(.45,1.05,length(q)));
 col=mix(col,u_c3,smoothstep(.5,.95,r.y)*.85);
 col=mix(col,u_c0,smoothstep(.62,.9,f)*.55);
 float grain=hash(gl_FragCoord.xy+fract(u_time*7.)*91.)-.5;
 col+=grain*.05;
 gl_FragColor=vec4(col,1.);
}`;

function rgb(hex: string) {
  const n = parseInt(hex.slice(1), 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255] as const;
}

export default function Aurora({ palette, seed = 0 }: { palette: Palette; seed?: number }) {
  const host = useRef<HTMLDivElement>(null),
    canvas = useRef<HTMLCanvasElement>(null),
    clock = useRef(12 + seed * 37);
  const [paused, setPaused] = useState(false),
    [reduced, setReduced] = useState(true);
  const colors = palettes[palette];
  useEffect(() => {
    const media = matchMedia('(prefers-reduced-motion: reduce)');
    const update = () => setReduced(media.matches);
    update();
    media.addEventListener('change', update);
    return () => media.removeEventListener('change', update);
  }, []);
  useEffect(() => {
    const surface = canvas.current,
      root = host.current;
    if (!surface || !root) return;
    const gl = surface.getContext('webgl', { alpha: false, antialias: false });
    if (!gl) {
      root.dataset.renderer = 'fallback';
      return;
    }
    const shaders: WebGLShader[] = [];
    let program: WebGLProgram | null = null,
      buffer: WebGLBuffer | null = null,
      frame = 0,
      visible = true,
      last = 0,
      lastPaint = 0,
      width = 0,
      height = 0;
    const moving = !paused && !reduced;
    root.dataset.motion = moving ? 'playing' : 'paused';
    try {
      for (const [kind, source] of [
        [gl.VERTEX_SHADER, vertex],
        [gl.FRAGMENT_SHADER, fragment],
      ] as const) {
        const shader = gl.createShader(kind);
        if (!shader) throw new Error('Shader unavailable');
        shaders.push(shader);
        gl.shaderSource(shader, source);
        gl.compileShader(shader);
        if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS))
          throw new Error('Shader unavailable');
      }
      program = gl.createProgram();
      if (!program) throw new Error('Program unavailable');
      shaders.forEach((shader) => gl.attachShader(program!, shader));
      gl.linkProgram(program);
      if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw new Error('Program unavailable');
      gl.useProgram(program);
      buffer = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
      gl.bufferData(
        gl.ARRAY_BUFFER,
        new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]),
        gl.STATIC_DRAW,
      );
      const position = gl.getAttribLocation(program, 'a_position');
      gl.enableVertexAttribArray(position);
      gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 0, 0);
      const resolution = gl.getUniformLocation(program, 'u_res'),
        time = gl.getUniformLocation(program, 'u_time');
      colors.forEach((hex, i) =>
        gl.uniform3fv(gl.getUniformLocation(program!, `u_c${i}`), rgb(hex)),
      );
      const paint = (stamp: number) => {
        frame = 0;
        if (document.hidden || !visible || gl.isContextLost()) {
          last = 0;
          return;
        }
        if (moving && last) clock.current += Math.min((stamp - last) / 1000, 0.06);
        last = stamp;
        // About 30 frames a second is plenty for a slow field, and half the work.
        if (!moving || stamp - lastPaint > 32) {
          lastPaint = stamp;
          const rect = root.getBoundingClientRect(),
            dpr = Math.min(devicePixelRatio || 1, 1),
            w = Math.max(1, Math.round(rect.width * dpr)),
            h = Math.max(1, Math.round(rect.height * dpr));
          if (width !== w || height !== h) {
            width = surface.width = w;
            height = surface.height = h;
            gl.viewport(0, 0, w, h);
          }
          gl.uniform2f(resolution, width, height);
          gl.uniform1f(time, clock.current);
          gl.drawArrays(gl.TRIANGLES, 0, 6);
          root.dataset.renderer = 'webgl';
        }
        if (moving) frame = requestAnimationFrame(paint);
      };
      const resume = () => {
        if (!frame) frame = requestAnimationFrame(paint);
      };
      const visibility = () => {
        if (document.hidden) {
          cancelAnimationFrame(frame);
          frame = 0;
          last = 0;
        } else resume();
      };
      const lost = (event: Event) => {
        event.preventDefault();
        cancelAnimationFrame(frame);
        frame = 0;
        root.dataset.renderer = 'fallback';
      };
      const observer =
        typeof IntersectionObserver !== 'undefined'
          ? new IntersectionObserver(([entry]) => {
              visible = entry.isIntersecting;
              if (visible) resume();
              else {
                cancelAnimationFrame(frame);
                frame = 0;
                last = 0;
              }
            })
          : null;
      observer?.observe(root);
      const resize = typeof ResizeObserver !== 'undefined' ? new ResizeObserver(resume) : null;
      resize?.observe(root);
      document.addEventListener('visibilitychange', visibility);
      surface.addEventListener('webglcontextlost', lost);
      resume();
      return () => {
        cancelAnimationFrame(frame);
        observer?.disconnect();
        resize?.disconnect();
        document.removeEventListener('visibilitychange', visibility);
        surface.removeEventListener('webglcontextlost', lost);
        if (buffer) gl.deleteBuffer(buffer);
        if (program) gl.deleteProgram(program);
        shaders.forEach((shader) => gl.deleteShader(shader));
      };
    } catch {
      root.dataset.renderer = 'fallback';
      if (buffer) gl.deleteBuffer(buffer);
      if (program) gl.deleteProgram(program);
      shaders.forEach((shader) => gl.deleteShader(shader));
    }
  }, [paused, reduced, colors]);
  return (
    <div
      className="m-aurora"
      ref={host}
      style={{
        background: `radial-gradient(60% 80% at 20% 20%, ${colors[1]}, transparent), radial-gradient(60% 70% at 85% 30%, ${colors[2]}, transparent), radial-gradient(70% 80% at 60% 100%, ${colors[3]}, transparent), ${colors[0]}`,
      }}
    >
      <canvas ref={canvas} aria-hidden="true" />
      <button
        type="button"
        className="m-motion"
        aria-label={
          reduced
            ? 'Animation reduced by your preference'
            : paused
              ? 'Play background animation'
              : 'Pause background animation'
        }
        aria-pressed={!paused && !reduced}
        disabled={reduced}
        onClick={() => setPaused((value) => !value)}
      >
        {paused || reduced ? <Play size={10} /> : <Pause size={10} />}
      </button>
    </div>
  );
}
