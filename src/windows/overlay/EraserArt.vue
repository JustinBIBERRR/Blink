<template>
<svg viewBox="0 0 180 72" aria-hidden="true">
  <defs>
    <linearGradient id="rim" x2="0" y2="1"><stop stop-color="#8495a1"/><stop offset="1" stop-color="#35424d"/></linearGradient>
    <linearGradient id="well" x2="0" y2="1"><stop stop-color="#28323b"/><stop offset="1" stop-color="#586976"/></linearGradient>
    <linearGradient id="top" x2=".2" y2="1"><stop stop-color="#c2dce9"/><stop offset="1" stop-color="#819fb2"/></linearGradient>
    <linearGradient id="body" x2="0" y2="1"><stop stop-color="#5c6b77"/><stop offset="1" stop-color="#283540"/></linearGradient>
    <filter id="felt"><feTurbulence baseFrequency=".75" numOctaves="3" seed="12"/><feColorMatrix values="0 0 0 0 .52 0 0 0 0 .55 0 0 0 0 .56 0 0 0 .6 0"/><feComposite in2="SourceGraphic" operator="in"/><feBlend in2="SourceGraphic" mode="multiply"/></filter>
  </defs>
  <path v-if="!lifted" d="M13 29 Q15 23 25 23H154Q165 23 167 30L176 56Q178 69 165 70H14Q2 69 4 57Z" fill="url(#rim)" stroke="#afc1cc" stroke-width=".7"/>
  <path v-if="!lifted" d="M20 30Q21 28 28 28H150Q158 28 160 33L167 54Q169 61 159 62H21Q11 62 14 53Z" fill="url(#well)" stroke="#26323c"/>
  <g v-if="!empty">
    <path d="M25 41H153L156 54Q155 59 149 60H30Q24 59 23 54Z" fill="#9b9c97" filter="url(#felt)"/>
    <path d="M26 14Q29 9 38 9H140Q151 9 154 17L160 43Q161 51 151 52H28Q18 51 20 43Z" fill="url(#body)" stroke="#6c7e8c"/>
    <path d="M32 10Q35 4 44 4H135Q145 4 148 11L153 29Q155 36 146 37H33Q24 36 26 29Z" fill="url(#top)" stroke="#d0e4ee" stroke-width=".7"/>
    <path d="M35 38H145" stroke="#1d2d3a" opacity=".6"/>
  </g>
</svg>
</template>
<script setup lang="ts">defineProps<{empty: boolean; lifted?: boolean}>();</script>
