// The one place a `ListSymbol` becomes an icon: the Lucide icon the web UI
// draws each of the daemon's list symbols with. The iPhone app keeps its own
// map to SF Symbols; "List symbols" in docs/architecture.md lists both.

import Banknote from '@lucide/svelte/icons/banknote';
import BookOpen from '@lucide/svelte/icons/book-open';
import Bookmark from '@lucide/svelte/icons/bookmark';
import Briefcase from '@lucide/svelte/icons/briefcase';
import BriefcaseMedical from '@lucide/svelte/icons/briefcase-medical';
import Bug from '@lucide/svelte/icons/bug';
import Calendar from '@lucide/svelte/icons/calendar';
import Camera from '@lucide/svelte/icons/camera';
import Car from '@lucide/svelte/icons/car';
import Code from '@lucide/svelte/icons/code';
import Dumbbell from '@lucide/svelte/icons/dumbbell';
import Flag from '@lucide/svelte/icons/flag';
import Gift from '@lucide/svelte/icons/gift';
import Globe from '@lucide/svelte/icons/globe';
import GraduationCap from '@lucide/svelte/icons/graduation-cap';
import Heart from '@lucide/svelte/icons/heart';
import House from '@lucide/svelte/icons/house';
import Inbox from '@lucide/svelte/icons/inbox';
import Leaf from '@lucide/svelte/icons/leaf';
import Lightbulb from '@lucide/svelte/icons/lightbulb';
import Lock from '@lucide/svelte/icons/lock';
import Mail from '@lucide/svelte/icons/mail';
import MessageCircle from '@lucide/svelte/icons/message-circle';
import Music from '@lucide/svelte/icons/music';
import PawPrint from '@lucide/svelte/icons/paw-print';
import Plane from '@lucide/svelte/icons/plane';
import ShoppingCart from '@lucide/svelte/icons/shopping-cart';
import Star from '@lucide/svelte/icons/star';
import User from '@lucide/svelte/icons/user';
import Users from '@lucide/svelte/icons/users';
import Utensils from '@lucide/svelte/icons/utensils';
import Wrench from '@lucide/svelte/icons/wrench';
import type { Component } from 'svelte';
import type { ListSymbol } from '$lib/protocol/types';

/** The icon each list symbol is drawn with. */
export const listSymbolIcon: Record<ListSymbol, Component> = {
	home: House,
	work: Briefcase,
	inbox: Inbox,
	star: Star,
	heart: Heart,
	flag: Flag,
	bookmark: Bookmark,
	book: BookOpen,
	cart: ShoppingCart,
	money: Banknote,
	calendar: Calendar,
	people: Users,
	person: User,
	health: BriefcaseMedical,
	fitness: Dumbbell,
	travel: Plane,
	car: Car,
	food: Utensils,
	code: Code,
	idea: Lightbulb,
	music: Music,
	camera: Camera,
	gift: Gift,
	school: GraduationCap,
	tools: Wrench,
	garden: Leaf,
	pets: PawPrint,
	bug: Bug,
	globe: Globe,
	chat: MessageCircle,
	mail: Mail,
	lock: Lock
};
