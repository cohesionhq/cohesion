<!--
	A soft arrow on a floating card's edge, pointing at what opened it, drawn as one surface with the
	card: bits-ui sets it at the card's outer edge, so its fill reaches back over the card's border
	where they meet and the border runs on down both slopes. bits-ui turns it toward the trigger.
	Put it last inside a bits-ui `Content` (every primitive's arrow is the same one underneath) whose
	surface doesn't clip what overflows it, and spread `cardArrowPlacement` onto that `Content`.
-->
<script module lang="ts">
	/**
	 * Where a card with an arrow sits: further off its trigger, leaving room for the arrow, and
	 * starting a little before it, so the arrow meets a straight stretch of the card's border
	 * rather than a rounded corner.
	 */
	export const cardArrowPlacement = { sideOffset: 16, alignOffset: -12, arrowPadding: 12 };
</script>

<script lang="ts">
	import { LinkPreview } from 'bits-ui';
</script>

<LinkPreview.Arrow width={26} height={12}>
	<svg width="26" height="12" viewBox="0 0 26 12" class="block overflow-visible" aria-hidden="true">
		<path
			d="M0 -2 H26 V-0.5 C21 -0.5 18 1 15.5 5.5 Q13 10 10.5 5.5 C8 1 5 -0.5 0 -0.5 Z"
			fill="var(--bg-floating)"
		/>
		<path
			d="M0 -0.5 C5 -0.5 8 1 10.5 5.5 Q13 10 15.5 5.5 C18 1 21 -0.5 26 -0.5"
			fill="none"
			stroke="var(--border)"
			stroke-width="1"
		/>
	</svg>
</LinkPreview.Arrow>
