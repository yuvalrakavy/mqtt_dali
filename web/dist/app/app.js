// node_modules/preact/dist/preact.mjs
var e, t, n, r, i, a, o, s, c, l, u, d = {}, f = [], p = /^m(i|n|o|s|text|space)$/, m = Array.isArray, h = f.slice, g = Object.assign;
function _(e) {
	e && e.parentNode && e.remove();
}
function v(n, r, i, a, o) {
	var s = {
		type: n,
		props: r,
		key: i,
		ref: a,
		__k: null,
		__: null,
		__b: 0,
		__e: null,
		__c: null,
		constructor: void 0,
		__v: o || ++t,
		__i: -1,
		__u: 0
	};
	return !o && e.vnode && e.vnode(s), s;
}
function y(e) {
	return e.children;
}
function b(e, t) {
	this.props = e, this.context = t, this.__g = 0;
}
function x(e, t) {
	if (t == null) return e.__ ? x(e.__, e.__i + 1) : null;
	for (var n; t < e.__k.length; t++) if ((n = e.__k[t]) && n.__e) return n.__e;
	return typeof e.type != "function" || e.props.__P ? null : x(e);
}
function S(e) {
	if ((e = e.__) && e.__c && !e.props.__P) return e.__e = null, e.__k.some(function(t) {
		return t && (e.__e = t.__e);
	}), S(e);
}
function C(t) {
	(8 & t.__g || !(t.__g |= 8) || !n.push(t) || i++) && r == e.debounceRendering || ((r = e.debounceRendering) || queueMicrotask)(w);
}
function w() {
	var t, r, o, s, c, l, u, d, f;
	try {
		for (r = 1; n.length;) n.length > r && n.sort(a), t = n.shift(), r = n.length, 8 & t.__g && (s = void 0, c = void 0, l = (c = (o = t).__v).__e, u = [], d = [], (f = o.__P) && ((s = g({ constructor: void 0 }, c)).__v = c.__v + 1, e.vnode && e.vnode(s), M(f, s, c, o.__n, f.namespaceURI, 32 & c.__u ? [l] : null, u, l || x(c), 32 & c.__u, d), s.__v = c.__v, s.__.__k[s.__i] = s, N(u, s, d), c.__ = c.__e = null, s.__e != l && S(s)));
	} finally {
		n.length = i = 0;
	}
}
function T(e, t, n, r, i, a, o, s, c, l, u) {
	var p, m, h, g, _, v, y = r.__k || f, b = t.length;
	for (c = E(n, t, y, c, b), p = 0; p < b; p++) (h = n.__k[p]) != null && (m = ~h.__i && y[h.__i] || d, h.__i = p, v = M(e, h, m, i, a, o, s, c, l, u), g = h.__e, (m.ref != h.ref || 8 & m.__u) && (m.ref && P(m.ref, null, h, m), h.ref && u.push(h.ref, h.__c || g, h)), _ = _ || g, 4 & h.__u ? (c = D(h, c, e, !m.__v), m.__e && (m.__e = null)) : typeof h.type == "function" && v !== void 0 ? c = v : g && (c = g.nextSibling), h.__u &= -7);
	return n.__e = _, c;
}
function E(e, t, n, r, i) {
	var a, o, s, c, l, u, d, f, p, h, g = n.length, _ = g, b = 0, S = !1, C = e.__k = Array(i);
	for (a = 0; a < i; a++) (o = t[a]) != null && typeof o != "boolean" && typeof o != "function" ? (typeof o != "object" || o.constructor == String ? o = C[a] = v(null, o) : m(o) ? o = C[a] = v(y, { children: o }) : o.constructor === void 0 && o.__b ? o = C[a] = v(o.type, o.props, o.key, o.ref, o.__v) : C[a] = o, c = a + b, o.__ = e, o.__b = e.__b + 1, s = null, ~(l = o.__i = O(o, n, c, _)) && (_--, (s = n[l]) && (s.__u |= 2)), s && s.__v ? (o.__u |= 2, l == c - 1 ? b-- : l == c + 1 ? b++ : l != c && (l > c ? b-- : b++, S = !0)) : (~l || (i > g ? b-- : i < g && b++), typeof o.type != "function" && (o.__u |= 4))) : C[a] = null;
	if (S) {
		for (u = [], d = [], a = 0; a < i; a++) if ((o = C[a]) && 2 & o.__u) {
			for (f = 0, p = u.length; f < p;) u[h = f + p >> 1] < o.__i ? f = h + 1 : p = h;
			u[f] = o.__i, d[a] = f + 1;
		}
		for (b = u.length; a--;) d[a] && (d[a] == b ? b-- : C[a].__u |= 4);
	}
	if (_) for (a = 0; a < g; a++) !(s = n[a]) || 2 & s.__u || (s.__e == r && (r = x(s)), F(s, s));
	return r;
}
function D(e, t, n, r) {
	var i, a, o;
	if (typeof e.type == "function") {
		if (e.props.__P) return t;
		if (i = e.__k) for (a = 0; a < i.length; a++) i[a] && (i[a].__ = e, t = D(i[a], t, n, !1));
		return t;
	}
	for (t && !t.parentNode && (t = x(e)) && !t.parentNode && (t = null), o = t; o && o.nodeType == 8;) o = o.nextSibling;
	for (e.__e != o && (!r && n.moveBefore && e.__e.parentNode ? n.moveBefore(e.__e, t) : n.insertBefore(e.__e, t || null)), t = e.__e; (t = t && t.nextSibling) && t.nodeType == 8;);
	return t;
}
function O(e, t, n, r) {
	var i, a, o, s = e.key, c = e.type, l = t[n], u = l && !(2 & l.__u);
	if (l === null && s == null || u && s == l.key && c == l.type) return n;
	if (r > +!!u) {
		for (i = n - 1, a = n + 1; i >= 0 || a < t.length;) if ((l = t[o = i >= 0 ? i-- : a++]) && !(2 & l.__u) && s == l.key && c == l.type) return o;
	}
	return -1;
}
function k(e, t, n) {
	n ?? (n = ""), t[0] == "-" ? e.setProperty(t, n) : e[t] = n;
}
function A(e, t, n, r, i) {
	var a, o;
	n: if (t == "style") {
		if (typeof n == "string") e.style.cssText = n;
		else {
			if (typeof r == "string" && (e.style.cssText = r = ""), r) for (t in r) n && t in n || k(e.style, t, "");
			if (n) for (t in n) r && n[t] == r[t] || k(e.style, t, n[t]);
		}
	} else if (t[0] == "o" && t[1] == "n") (e.__e || (e.__e = {}))[t] = n, n && r || (o = l[t] || (l[t] = j(t)), (e.__a || (e.__a = {}))[t] = c, a = t != (t = t.replace(s, "$1")), (t = t.slice(2))[0] < "a" && (t = t.toLowerCase()), n ? e.addEventListener(t, o, a) : e.removeEventListener(t, o, a));
	else {
		if (i == "http://www.w3.org/2000/svg") t = t.replace(/xlink(H|:h)/, "h").replace(/sName$/, "s");
		else if (t != "width" && t != "height" && t != "href" && t != "list" && t != "form" && t != "tabIndex" && t != "download" && t != "rowSpan" && t != "colSpan" && t != "role" && t != "popover" && t in e) try {
			e[t] = n ?? "";
			break n;
		} catch {}
		typeof n == "function" || (n == null || !1 === n && t[4] != "-" ? e.removeAttribute(t) : e.setAttribute(t, t == "popover" && n == 1 ? "" : n));
	}
}
function j(t) {
	return function(n) {
		if (this.__e) {
			var r = this.__e[t];
			if (n[o] == null) n[o] = c++;
			else if (n[o] < this.__a[t]) return;
			return r(e.event ? e.event(n) : n);
		}
	};
}
function M(t, n, r, i, a, o, s, c, l, u) {
	var d, p, h, v, S, C, w, E, D, O, k, A, j, M, N, P, I, L, R, z, B = n.type;
	if (n.constructor !== void 0) return null;
	if (128 & r.__u && (l = 32 & r.__u, d = r.__c.__z)) {
		if (n.__u |= l, p = o = [], d.nodeType == 8) for (h = 1, v = d.nextSibling; v; v = v.nextSibling) {
			if (v.nodeType == 8) {
				if (v.data.startsWith("$s")) h++;
				else if (v.data.startsWith("/$s") && !--h) break;
			}
			o.push(v);
		}
		else o.push(d);
		c = o[0];
	}
	(d = e.__b) && d(n);
	n: if (typeof B == "function") {
		S = s.length;
		try {
			if (O = n.props, k = (d = B.prototype) && d.render, A = (d = B.contextType) && i[d.__c], j = d ? A ? A.props.value : d.__ : i, r.__c ? 2 & (C = n.__c = r.__c).__g && (C.__g |= 1) : (k ? n.__c = C = new B(O, j) : (n.__c = C = new b(O, j), C.constructor = B, C.render = ne), A && A.sub(C), C.state || (C.state = {}), C.__n = i, C.__g |= 8, C.__h = [], C.__k = []), k && (C.__s || (C.__s = C.state), B.getDerivedStateFromProps && (C.__s == C.state && (C.__s = g({}, C.__s)), g(C.__s, B.getDerivedStateFromProps(O, C.__s)))), w = C.props, E = C.state, C.__v = n, r.__c) {
				if (k && !B.getDerivedStateFromProps && O !== w && C.componentWillReceiveProps && C.componentWillReceiveProps(O, j), n.__v == r.__v && !(8 & C.__g) || !(4 & C.__g) && C.shouldComponentUpdate && !1 === C.shouldComponentUpdate(O, C.__s, j)) {
					n.__v != r.__v && (C.props = O, C.state = C.__s, C.__g &= -9), n.__e = r.__e, n.__k = r.__k, n.__k.some(function(e) {
						e && (e.__ = n);
					}), f.push.apply(C.__h, C.__k), C.__k = [], C.__h.length && s.push(C), c = x(r);
					break n;
				}
				C.componentWillUpdate && C.componentWillUpdate(O, C.__s, j), k && C.componentDidUpdate && C.__h.push(function() {
					C.componentDidUpdate(w, E, D);
				});
			} else k && !B.getDerivedStateFromProps && C.componentWillMount && C.componentWillMount(), k && C.componentDidMount && C.__h.push(C.componentDidMount);
			if (C.context = j, C.props = O, C.__P = t, C.__g &= -5, M = e.__r, N = 0, k) C.state = C.__s, C.__g &= -9, M && M(n), d = C.render(C.props, C.state, C.context), f.push.apply(C.__h, C.__k), C.__k = [];
			else do
				C.__g &= -9, M && M(n), d = C.render(C.props, C.state, C.context), C.state = C.__s;
			while (8 & C.__g && ++N < 25);
			C.state = C.__s, C.getChildContext && (i = g({}, i, C.getChildContext())), k && r.__c && C.getSnapshotBeforeUpdate && (D = C.getSnapshotBeforeUpdate(w, E)), P = d && d.type === y && d.key == null ? d.props.children : d, O.__P && (d = c, a = (t = O.__P).namespaceURI, l = o = null, r.props && r.props.__P != t && (r.__k.some(function(e) {
				e && F(e, e);
			}), r.__k = null), c = r.__k ? x(r, 0) : null), c = T(t, m(P) ? P : [P], n, r, i, a, o, s, c, l, u), O.__P && (n.__e = null, c = d), n.__u &= -161, 128 & r.__u && (C.__z = null), p && p.some(_), C.__h.length && s.push(C), 1 & C.__g && (C.__g &= -4);
		} catch (t) {
			if (s.length = S, n.__v = null, l || o) {
				if (t.then) {
					if (I = 0, n.__u |= l ? 160 : 128, o) {
						for (~(R = o.indexOf(c || void 0)) || (R = o.length); (z = o[R - 1]) && z.nodeType == 8;) R--;
						for (; R < o.length; R++) if (z = o[R]) {
							if (o[R] = null, z.nodeType == 8) {
								if (z.data.startsWith("$s")) I++ || (L = z);
								else if (I && z.data.startsWith("/$s") && !--I) {
									c = z;
									break;
								}
							} else if (!I) break;
						}
					}
					if (!L) {
						for (; c && c.nodeType == 8 && c.nextSibling;) c = c.nextSibling;
						L = c;
					}
					n.__c.__z || (n.__c.__z = L), n.__e = c;
				} else o && o.some(_);
			} else n.__e = r.__e;
			n.__k || (n.__k = r.__k || []), t.then || ee(n), e.__e(t, n, r);
		}
	} else c = n.__e = te(r.__e, n, r, i, a, o, s, l, u, t);
	return (d = e.diffed) && d(n), 128 & n.__u ? void 0 : c;
}
function ee(e) {
	e && (e.__c && (e.__c.__g |= 4), e.__k && e.__k.some(ee));
}
function N(t, n, r) {
	for (var i = 0; i < r.length;) P(r[i++], r[i++], r[i++]);
	e.__c && e.__c(n, t), t.some(function(n) {
		try {
			t = n.__h, n.__h = [], t.some(function(e) {
				e.call(n);
			});
		} catch (t) {
			e.__e(t, n.__v);
		}
	});
}
function te(t, n, r, i, a, o, s, c, l, u) {
	var f, g, v, y, b, S, C, w, E, D = r.props || d, O = n.props, k = n.type;
	if (k == "svg" ? a = "http://www.w3.org/2000/svg" : k == "math" ? a = "http://www.w3.org/1998/Math/MathML" : a || (a = "http://www.w3.org/1999/xhtml"), o) {
		for (f = 0; f < o.length; f++) if ((b = o[f]) && (k ? b.localName == k : b.nodeType == 3)) {
			t = b, o[f] = null;
			break;
		}
	}
	if (!t) {
		if (w = u.ownerDocument || document, !k) return w.createTextNode(O);
		t = w.createElementNS(a, k, O.is && O), c && (e.__m && e.__m(n, o), c = !1), o = null;
	}
	if (k) {
		if (u = k == "template" ? t.content : t, o = k == "textarea" && O.defaultValue != null ? null : o && h.call(u.childNodes), !c && o) for (D = {}, f = 0; f < t.attributes.length; f++) D[(b = t.attributes[f]).name] = b.value;
		for (f in D) b = D[f], f == "dangerouslySetInnerHTML" ? v = b : f == "children" || f in O || f == "value" && "defaultValue" in O || f == "checked" && "defaultChecked" in O || A(t, f, null, b, a);
		for (f in E = 1 & r.__u, O) b = O[f], f == "children" ? y = b : f == "dangerouslySetInnerHTML" ? g = b : f == "value" ? S = b : f == "checked" ? C = b : c && typeof b != "function" || !(D[f] !== b || E && b != null) || A(t, f, b, D[f], a);
		g ? (c || v && (g.__html == v.__html || g.__html == t.innerHTML) || (t.innerHTML = g.__html), n.__k = []) : (v && (t.textContent = ""), (k == "foreignObject" || a == "http://www.w3.org/1998/Math/MathML" && p.test(k)) && (a = "http://www.w3.org/1999/xhtml"), T(u, m(y) ? y : [y], n, r, i, a, o, s, o ? o[0] : r.__k && x(r, 0), c, l), o && o.some(_)), c && k != "textarea" || (f = "value", k == "progress" && S == null ? t.removeAttribute(f) : S == null || S === t[f] && (k != "progress" || S) || A(t, f, S, D[f], a), f = "checked", C != null && C != t[f] && A(t, f, C, D[f], a));
	} else D === O || c && t.data == O || (t.data = O);
	return t;
}
function P(t, n, r, i) {
	try {
		typeof t == "function" ? n ? n.__x = t(n) || 1 : i && (i = i.__c || i.__e) && (n = i.__x) && (i.__x = null, typeof n == "function" ? n() : t(null)) : t.current = n;
	} catch (t) {
		e.__e(t, r);
	}
}
function F(t, n, r) {
	var i, a;
	if (e.unmount && e.unmount(t), !(i = t.ref) || i.current && i.current != t.__e || P(i, null, n, t), i = t.__c) {
		if (i.componentWillUnmount) try {
			i.componentWillUnmount();
		} catch (t) {
			e.__e(t, n);
		}
		i.__P = i.__n = null;
	}
	if (i = t.__k) for (a = 0; a < i.length; a++) i[a] && F(i[a], n, typeof t.type != "function" || r && !t.props.__P);
	(i = t.__e) && (r || _(i), i.__e && (i.__e = null)), t.__e = t.__c = t.__ = null;
}
function ne(e, t, n) {
	return this.constructor(e, n);
}
function I(t, n) {
	var r, i, a, o;
	e.__ && e.__(t, n), n.nodeType == 9 && (n = n.documentElement), i = (r = t && 32 & t.__u) ? null : n.__k, n.__k = v(y, { children: [t] }), a = [], o = [], M(n, n.__k, i || d, d, n.namespaceURI, i ? null : n.firstChild ? h.call(n.childNodes) : null, a, i ? i.__e : n.firstChild, r, o), N(a, n.__k, o), n.__k.props.children = null;
}
function L(e) {
	function t(e) {
		var n, r;
		return this.getChildContext || (n = /* @__PURE__ */ new Set(), (r = {})[t.__c] = this, this.getChildContext = function() {
			return r;
		}, this.shouldComponentUpdate = function(e) {
			this.props.value != e.value && n.forEach(function(e) {
				e.__g |= 4, C(e);
			});
		}, this.sub = function(e) {
			n.add(e);
			var t = e.componentWillUnmount;
			e.componentWillUnmount = function() {
				n.delete(e), t && t.call(e);
			};
		}), e.children;
	}
	return t.__c = "__cC" + u++, t.__ = e, t.Provider = (t.Consumer = function(e, t) {
		return e.children(t);
	}).contextType = t, t;
}
e = { __e: function(e, t, n, r) {
	for (var a, o, s; t = t.__;) if ((a = t.__c) && !(1 & a.__g)) {
		a.__g |= 4;
		try {
			if ((o = a.constructor) && o.getDerivedStateFromError && (a.setState(o.getDerivedStateFromError(e)), s = 8 & a.__g), a.componentDidCatch && (a.componentDidCatch(e, r || {}), s = 8 & a.__g), s) return void (a.__g |= 2);
		} catch (t) {
			e = t, s = 0;
		}
	}
	throw i = 0, e;
} }, t = 0, b.prototype.setState = function(e, t) {
	var n = this.__s;
	n && n != this.state || (n = this.__s = g({}, this.state)), typeof e == "function" && (e = e(g({}, n), this.props)), e && (g(n, e), this.__v && (t && this.__k.push(t), C(this)));
}, b.prototype.forceUpdate = function(e) {
	this.__v && (this.__g |= 4, e && this.__h.push(e), C(this));
}, b.prototype.render = y, n = [], i = 0, a = function(e, t) {
	return e.__v.__b - t.__v.__b;
}, o = Symbol(), s = /(PointerCapture)$|Capture$/i, c = 0, l = {}, u = 0;
// node_modules/preact/hooks/dist/hooks.mjs
var R, z, B, re, ie = Object.is, ae = 0, oe = [], se = [], V = e, ce = V.__b, le = V.__r, ue = V.diffed, de = V.__c, fe = V.unmount, pe = V.__;
function me(e, t) {
	V.__h && V.__h(z, e, ae || t), ae = 0;
	var n = z.__H || (z.__H = {
		__: [],
		__h: []
	});
	return e >= n.__.length && n.__.push({}), n.__[e];
}
function H(e) {
	return ae = 1, he(De, e);
}
function he(e, t, n) {
	var r = me(R++, 2);
	if (r.t = e, !r.__c && (r.__ = [n ? n(t) : De(void 0, t), function(e) {
		var t = r.__N ? r.__N[0] : r.__[0], n = r.t(t, e);
		ie(t, n) || (r.__N = [n, r.__[1]], r.__c.setState({}));
	}], r.__c = z, !z.__f)) {
		z.__f = !0;
		var i = z.shouldComponentUpdate;
		z.shouldComponentUpdate = function(e, t, n) {
			var r = this.__H;
			if (!r) return !0;
			var a = !1, o = this.props != e;
			if (r.__.some(function(e) {
				e.__N && (a = !0, ie(e.__[0], e.__N[0]) || (o = !0));
			}), i) {
				var s = i.call(this, e, t, n);
				return a ? s || o : s;
			}
			return !a || o;
		};
	}
	return r.__;
}
function U(e, t) {
	var n = me(R++, 3);
	!V.__s && Ee(n.__H, t) && (n.__P = !0, n.__ = e, n.u = t, z.__H.__h.push(n));
}
function ge(e, t) {
	var n = me(R++, 4);
	!V.__s && Ee(n.__H, t) && (n.__P = !1, n.__ = e, n.u = t, z.__h.push(n));
}
function _e(e) {
	return ae = 5, ve(function() {
		return { current: e };
	}, []);
}
function ve(e, t) {
	var n = me(R++, 7);
	return Ee(n.__H, t) && (n.__ = e(), n.__H = t), n.__;
}
function ye(e) {
	var t = z.context[e.__c], n = me(R++, 9);
	return n.c = e, t ? (n.__ ?? (n.__ = !0, t.sub(z)), t.props.value) : e.__;
}
function be() {
	var e;
	do {
		for (; e = se.shift();) try {
			we(e);
		} catch (t) {
			V.__e(t, { __: (e = e.__P) && e.__v });
		}
		for (; e = oe.shift();) {
			var t = e.__H;
			if (e.__P && t) try {
				t.__h.some(we), t.__h.some(Te), t.__h = [];
			} catch (n) {
				t.__h = [], V.__e(n, e.__v);
			}
		}
	} while (se.length);
}
V.__b = function(e) {
	z = null, ce && ce(e);
}, V.__ = function(e, t) {
	e && t.__k && t.__k.__m && (e.__m = t.__k.__m), pe && pe(e, t);
}, V.__r = function(e) {
	le && le(e), R = 0;
	var t = (z = e.__c).__H;
	t && (B == z ? z.__h = [] : (t.__h.some(we), t.__h.some(Te), R = 0), t.__h = [], t.__.some(function(e) {
		e.__N && (e.__ = e.__N), e.u = e.__N = void 0;
	})), B = z;
}, V.diffed = function(e) {
	ue && ue(e);
	var t = e.__c;
	t && t.__H && (t.__H.__h.length && Ce(oe.push(t)), t.__H.__.some(function(e) {
		e.u && (e.__H = e.u);
	})), B = z = null;
}, V.__c = function(e, t) {
	t.some(function(e) {
		try {
			e.__h.some(we), e.__h = e.__h.filter(function(e) {
				return !e.__ || Te(e);
			});
		} catch (n) {
			t.some(function(e) {
				e.__h && (e.__h = []);
			}), t = [], V.__e(n, e.__v);
		}
	}), de && de(e, t);
}, V.unmount = function(e) {
	fe && fe(e);
	var t, n, r = e.__c;
	r && r.__H && (r.__H.__.some(function(r) {
		try {
			if (r.__P && r.__c) {
				if (n === void 0) {
					for (n = e.__; n && (!n.__c || !n.__c.__P);) n = n.__;
					n = n && n.__c;
				}
				r.__P = n, Ce(se.push(r));
			} else we(r);
		} catch (e) {
			t = e;
		}
	}), r.__H = void 0, t && V.__e(t, r.__v));
};
var xe = typeof requestAnimationFrame == "function";
function Se(e) {
	var t, n = function() {
		clearTimeout(r), xe && cancelAnimationFrame(t), setTimeout(e);
	}, r = setTimeout(n, 35);
	xe && (t = requestAnimationFrame(n));
}
function Ce(e) {
	e != 1 && re == V.requestAnimationFrame || ((re = V.requestAnimationFrame) || Se)(be);
}
function we(e) {
	var t = z, n = e.__c;
	typeof n == "function" && (e.__c = void 0, n()), z = t;
}
function Te(e) {
	var t = z;
	e.__c = e.__(), z = t;
}
function Ee(e, t) {
	return !e || e.length != t.length || t.some(function(t, n) {
		return !ie(t, e[n]);
	});
}
function De(e, t) {
	return typeof t == "function" ? t(e) : t;
}
// src/model.ts
function W(e, t) {
	if (!/^[a-f0-9]{32}$/.test(e) || !Number.isSafeInteger(t) || t < 1) throw Error("Invalid inventory authority; refresh the controller.");
	return `${e}.${t}`;
}
function Oe(e, t) {
	let n = /* @__PURE__ */ new Map(), r = (e) => `${t}:${e.binding_revision}:${e.target}`;
	for (let t of e.baseline) {
		if (n.has(t.target)) throw Error("Duplicate inventory identity");
		n.set(t.target, {
			key: r(t),
			record: t
		});
	}
	for (let t of e.batches) {
		let e = /* @__PURE__ */ new Map();
		t.transitions.forEach((t, r) => {
			if (!t.from && !t.to) throw Error("Empty inventory transition");
			if (t.from) {
				let i = n.get(t.from.target);
				if (!i || i.record.binding_revision !== t.from.binding_revision || JSON.stringify(i.record.location) !== JSON.stringify(t.from.location)) throw Error("Incomplete inventory history");
				e.set(r, i.key);
			}
		});
		for (let e of t.transitions) e.from && n.delete(e.from.target);
		t.transitions.forEach((t, i) => {
			if (t.to) {
				if (n.has(t.to.target)) throw Error("Conflicting inventory identity");
				n.set(t.to.target, {
					key: e.get(i) ?? r(t.to),
					record: t.to
				});
			}
		});
	}
	return new Map([...n].map(([e, t]) => [e, t.key]));
}
function G(e) {
	let t = e.inventory;
	W(t.inventory_epoch, t.revision);
	let n = Oe(t.identity_ledger, t.inventory_epoch), r = [];
	for (let e of t.buses) {
		let i = (i, a, o) => {
			let s = n.get(a.target);
			if (!s) throw Error("Saved device has no identity history");
			r.push({
				key: s,
				kind: i,
				bus: e,
				item: a,
				parent: o,
				generation: W(t.inventory_epoch, a.binding_revision)
			});
		};
		for (let t of e.outputs) i("output", t);
		for (let t of e.groups ?? []) i("group", t);
		for (let i of e.scenes ?? []) {
			let a = n.get(i.target);
			if (!a) throw Error("Saved scene has no identity history");
			r.push({
				key: a,
				kind: "scene",
				bus: e,
				item: i,
				generation: W(t.inventory_epoch, i.binding_revision)
			});
		}
		for (let t of e.inputs ?? []) {
			i("input", t);
			for (let e of t.instances ?? []) i("instance", e, t);
		}
	}
	return r;
}
function ke(e, t) {
	if (!e) return !0;
	let n = e.replace(/[.+^${}()|[\]\\]/g, "\\$&").replace(/\*/g, ".*").replace(/\?/g, ".");
	return RegExp(`^${n}$`, "iu").test(t);
}
function K(e, t) {
	return e.phase === "ready" && t.enabled && e.hardware.state === "ready" && !!e.hardware.buses?.some((e) => e.bus === t.bus && e.status === 2);
}
function Ae(e, t) {
	return e.states.find((e) => e.target === t.item.target && e.generation === t.generation)?.values ?? {};
}
var je = (e) => ({
	1: "Part 301 · button",
	2: "Part 302 · input",
	3: "Part 303 · occupancy",
	4: "Part 304 · light sensor"
})[e ?? 0] ?? `Type ${e ?? "unknown"}`;
function Me(e) {
	return typeof e.Status == "boolean" ? e.Status ? "Pressed" : "Released" : typeof e.InputState == "boolean" ? e.InputState ? "On" : "Off" : typeof e.Occupied == "boolean" ? e.Occupied ? "Occupied" : "Vacant" : typeof e.InputValueRaw == "string" ? `${typeof e.Position == "number" ? `${Math.round(e.Position * 100)}% · ` : ""}0x${e.InputValueRaw} / ${e.ValueResolution} bit` : typeof e.PositionRaw == "number" ? `${e.PositionRaw} raw` : typeof e.LightLevelRaw == "number" ? `${e.LightLevelRaw} raw` : "Unknown";
}
var Ne = class {
	constructor(e, t, n) {
		this.error = "", this.text = this.saved = e, this.baseline = {
			target: t,
			generation: n
		};
	}
	get dirty() {
		return this.text !== this.saved;
	}
	commit(e) {
		if (this.pending) return this.pending;
		if (!this.dirty) return Promise.resolve();
		let t = this.text;
		return t.trim() ? (this.error = "", this.pending = Promise.resolve().then(() => e(t, this.baseline)).then(() => {
			this.saved = t;
		}).catch((e) => {
			throw this.error = e.message ?? "Name was not saved.", e;
		}).finally(() => {
			this.pending = void 0;
		}), this.pending) : (this.error = "A name is required.", Promise.reject(Error(this.error)));
	}
}, Pe = class {
	constructor() {
		this.epoch = 0, this.value = null, this.jobBook = [], this.ready = !1;
	}
	begin() {
		return this.epoch++, this.ready = !1, this.epoch;
	}
	snapshot(e, t) {
		e === this.epoch && (this.value = t, this.ready = !0);
	}
	jobs(e, t) {
		e === this.epoch && (this.jobBook = t);
	}
	clear() {
		this.begin(), this.value = null, this.jobBook = [];
	}
};
// src/client.ts
function Fe(e) {
	let t = {
		...e,
		inventory: e.inventory.Inventory
	};
	return G(t), t;
}
function q(e, t, n, r) {
	let i = e.inventory.buses.find((e) => e.bus === r), a = t.inventory.buses.find((e) => e.bus === r);
	return e.inventory.inventory_epoch === t.inventory.inventory_epoch && t.inventory.revision === n && !!i && !!a && i.name === a.name && a.enabled;
}
var Ie = class extends Error {
	constructor(e, t) {
		super(t), this.status = e;
	}
}, Le = class {
	constructor() {
		this.views = new Pe(), this.csrf = "", this.error = "", this.events = [], this.lost = 0, this.stopped = !1, this.session = 0, this.activeAt = 0, this.requests = /* @__PURE__ */ new Set(), this.sleepers = /* @__PURE__ */ new Map(), this.listeners = /* @__PURE__ */ new Set();
	}
	subscribe(e) {
		return this.listeners.add(e), () => this.listeners.delete(e);
	}
	notify() {
		for (let e of this.listeners) e();
	}
	pause(e) {
		return new Promise((t, n) => {
			let r = setTimeout(() => {
				this.sleepers.delete(r), t();
			}, e);
			this.sleepers.set(r, n);
		});
	}
	get snapshot() {
		return this.views.value;
	}
	get jobs() {
		return this.views.jobBook;
	}
	async request(e, t) {
		let n = this.session, r = new AbortController();
		this.requests.add(r);
		let i = setTimeout(() => r.abort(), 8e3);
		try {
			let i = await fetch(e, {
				method: t === void 0 ? "GET" : "POST",
				credentials: "same-origin",
				signal: r.signal,
				headers: t === void 0 ? {} : {
					"Content-Type": "application/json",
					"X-CSRF-Token": this.csrf
				},
				body: t === void 0 ? void 0 : JSON.stringify(t)
			});
			if (n !== this.session || this.stopped) throw Error("Session ended.");
			if (i.status === 401) throw this.expired(), new Ie(401, "Sign in required.");
			let a = i.status === 204 ? null : await i.json();
			if (n !== this.session || this.stopped) throw Error("Session ended.");
			if (!i.ok) throw new Ie(i.status, a?.error ?? "The controller refused this request.");
			return a;
		} finally {
			clearTimeout(i), this.requests.delete(r);
		}
	}
	async start() {
		let e = this.session, t = await this.request("/api/session");
		e !== this.session || this.stopped || (this.csrf = t.csrf, this.connect());
	}
	connect() {
		if (this.stopped) return;
		let e = this.views.begin(), t = this.session;
		this.source?.close();
		let n = new EventSource("/api/events");
		this.source = n;
		let r = () => !this.stopped && this.session === t && this.views.epoch === e && this.source === n;
		n.addEventListener("snapshot", (t) => {
			if (r()) try {
				this.views.snapshot(e, Fe(JSON.parse(t.data))), this.error = "", this.notify();
			} catch {
				this.error = "Invalid controller snapshot.", this.views.ready = !1, this.notify();
			}
		}), n.addEventListener("jobs", (t) => {
			if (r()) try {
				this.views.jobs(e, JSON.parse(t.data).jobs), this.notify();
			} catch {
				this.error = "Job updates are unavailable.", this.notify();
			}
		}), n.addEventListener("input", (e) => {
			if (r()) try {
				this.events.unshift({
					...JSON.parse(e.data),
					received: Date.now()
				}), this.events.length = Math.min(this.events.length, 64), this.notify();
			} catch {
				this.lost++, this.notify();
			}
		}), n.addEventListener("input_events_lost", (e) => {
			if (r()) {
				try {
					this.lost += JSON.parse(e.data).dropped;
				} catch {
					this.lost++;
				}
				this.notify();
			}
		}), n.onerror = () => {
			if (!r()) return;
			n.close(), this.source = void 0;
			let e = this.views.begin();
			this.error = "Connection interrupted. Reconnecting…", this.notify();
			let i = () => !this.stopped && this.session === t && this.views.epoch === e, a = async () => {
				if (i()) try {
					let e = await this.request("/api/session");
					i() && (this.csrf = e.csrf, this.connect());
				} catch {
					i() && (this.error = "Controller is unavailable.", this.notify(), this.reconnect = setTimeout(a, 2e3));
				}
			};
			this.reconnect = setTimeout(a, 1e3);
		};
	}
	activity() {
		let e = Date.now();
		e - this.activeAt < 6e4 || !this.csrf || this.stopped || (this.activeAt = e, this.request("/api/session/activity", {}).catch(() => {}));
	}
	async job(e, t, n, r = {}) {
		if (!this.views.ready) throw Error("Wait for a current controller view.");
		let i = this.session, a = (await this.request("/api/jobs", {
			generation: e,
			target: t,
			command: n,
			params: r
		})).id, o = Date.now() + 25e3;
		if (typeof a != "string") throw Error("Controller did not return a job id.");
		for (; Date.now() < o;) {
			if (i !== this.session || this.stopped) throw Error("Session ended; inspect the job before retrying.");
			let e = this.views.jobBook.find((e) => e.id === a);
			if ((!e || e.status === "running" || e.status === "succeeded" && e.result == null) && (e = await this.request(`/api/jobs/${a}`)), e?.status === "succeeded") return e.result;
			if (e?.status === "failed") throw Error(e.error?.message ?? "Operation failed. Inspect current state before retrying.");
			await this.pause(50);
		}
		throw Error("Result is not available yet. Inspect Jobs before retrying.");
	}
	async atRevision(e) {
		let t = Date.now() + 5e3, n = this.session;
		for (; Date.now() < t;) {
			if (this.stopped || n !== this.session) throw Error("Session ended.");
			let t = this.snapshot;
			if (this.views.ready && t && t.inventory.revision >= e) return t;
			await this.pause(20);
		}
		throw Error("Waiting for the updated inventory. Refresh before another change.");
	}
	async logout() {
		await this.request("/api/logout", {}), this.expired();
	}
	expired() {
		this.close(), location.assign("/login");
	}
	close() {
		this.stopped = !0, this.session++, this.source?.close(), this.reconnect && clearTimeout(this.reconnect);
		for (let e of this.requests) e.abort();
		this.requests.clear();
		for (let [e, t] of this.sleepers) clearTimeout(e), t(/* @__PURE__ */ Error("Session ended."));
		this.sleepers.clear(), this.views.clear(), this.csrf = "", this.events = [], this.notify();
	}
}, Re = class {
	constructor() {
		this.client = new Le(), this.drafts = /* @__PURE__ */ new Map(), this.hints = /* @__PURE__ */ new Map(), this.error = "", this.notice = "", this.busy = 0, this.progress = "", this.cancelled = !1, this.signingOut = !1, this.edits = Promise.resolve();
	}
	touch() {
		this.client.notify();
	}
	async logout() {
		if (!this.signingOut) {
			this.signingOut = !0, this.cancelled = !0, this.error = "", this.notice = "", this.touch();
			try {
				await this.client.logout(), this.drafts.clear();
			} catch (e) {
				this.error = e.message;
			} finally {
				this.signingOut = !1, this.touch();
			}
		}
	}
	get snapshot() {
		let e = this.client.snapshot;
		if (!e) throw Error("Wait for the controller inventory.");
		return e;
	}
	get currentRows() {
		return G(this.snapshot);
	}
	current(e) {
		let t = this.currentRows.find((t) => t.key === e);
		if (!t) throw Error("This fixture was removed or replaced.");
		return t;
	}
	physical(e) {
		let t = this.current(e);
		if (t.kind === "scene") throw Error("A physical device is required.");
		return t;
	}
	async run(e) {
		if (this.busy) this.error = "Wait for the current operation to finish.", this.touch();
		else {
			this.busy++, this.error = "", this.notice = "", this.touch();
			try {
				await e();
			} catch (e) {
				this.error = e.message;
			} finally {
				this.busy--, this.touch();
			}
		}
	}
	async changed(e) {
		return this.client.atRevision(e);
	}
	async metadata(e, t, n, r) {
		let i = await this.client.job(n, t, e, {
			...r,
			expected_revision: this.snapshot.inventory.revision
		});
		return await this.changed(i.revision), i;
	}
	rename(e, t, n) {
		let r = this.edits.catch(() => {}).then(async () => {
			if (this.progress) throw Error("Stop the current batch before saving a name.");
			this.busy++, this.touch();
			try {
				let r = this.current(e);
				if (r.item.target !== n.target || r.generation !== n.generation) throw Error("This device changed while you were editing. Discard the draft and review its name.");
				await this.metadata("rename", n.target, n.generation, { name: t });
				let i = this.current(e);
				if (i.item.name !== t) throw Error("The name changed again. Review the current device before retrying.");
				if (this.drafts.delete(e), this.notice = "Name saved.", i.kind === "output" || i.kind === "instance") try {
					await this.read(i);
				} catch {
					this.notice = "Name saved. Current values could not be refreshed; use Read current values.";
				}
			} finally {
				this.busy--, this.touch();
			}
		});
		return this.edits = r, r;
	}
	async read(e) {
		let t = this.current(e.key);
		await this.client.job(t.generation, t.item.target, t.kind === "instance" ? "read_input" : "read_output", {});
	}
	async apply(e, t) {
		let n = this.current(e.key);
		await this.client.job(n.generation, n.item.target, "apply_output", { values: t });
	}
	async blink(e) {
		let t = this.current(e.key), n = this.snapshot.identification;
		if (n && [
			"starting",
			"blinking",
			"restoring"
		].includes(n.phase) && n.target !== t.item.target) throw Error("Stop the other blinking fixture first.");
		await this.client.job(t.generation, t.item.target, n?.target === t.item.target && [
			"starting",
			"blinking",
			"restoring"
		].includes(n.phase) ? "stop_blink" : "blink", {});
	}
	async configureBus(e, t, n) {
		let r = this.snapshot;
		await this.metadata("configure_bus", null, W(r.inventory.inventory_epoch, r.inventory.revision), {
			bus: e,
			name: t,
			enabled: n
		}), this.notice = "Bus settings saved.";
	}
	async remove(e) {
		let t = this.current(e.key);
		await this.metadata("remove_from_list", t.item.target, t.generation, {}), this.notice = "Removed from the saved list. Hardware addresses are unchanged.";
	}
	async representation(e, t) {
		let n = this.current(e.key);
		await this.metadata("set_color_representation", n.item.target, n.generation, { color_representation: t || null }), await this.read(this.current(e.key)), this.notice = "Colour representation saved; actual values refreshed.";
	}
	hint(e) {
		let t = e.target;
		if (t) {
			let n = this.currentRows.find((e) => e.item.target === t);
			n && e.probe?.types_complete && this.hints.set(n.key, e.probe.device_types);
		}
		this.touch();
	}
	async scan(e, t) {
		this.cancelled = !1;
		let n = this.snapshot.inventory.revision, r = this.snapshot, i = async (t, i) => {
			if (this.cancelled) return null;
			if (!q(r, this.snapshot, n, e.bus)) throw Error("Bus configuration changed during discovery. Review before starting again.");
			let a = await this.client.job(W(r.inventory.inventory_epoch, n), null, t, {
				...i,
				expected_revision: n
			});
			if (!Number.isSafeInteger(a.revision)) throw Error("The controller returned an invalid inventory revision.");
			let o = await this.changed(a.revision);
			if (n = a.revision, !q(r, o, n, e.bus)) throw Error("Configuration changed during discovery. Completed steps remain saved.");
			return a.probe && this.hint(a), a;
		};
		try {
			for (let t of ["outputs", "inputs"]) for (let n = 0; n < 64 && !this.cancelled; n++) this.progress = `Discovering ${t} · address ${n}`, this.touch(), await i("discover_address", {
				bus: e.bus,
				space: t,
				address: n
			});
			if (t) for (let t of ["outputs", "inputs"]) for (let n = 0; n < 64 && !this.cancelled; n++) {
				this.progress = `Finding unaddressed ${t}`, this.touch();
				let n = await i(t === "outputs" ? "assign_next_output" : "assign_next_input", { bus: e.bus });
				if (!n) break;
				if (n.outcome === "no_free_address") throw Error(`No free ${t} addresses. Completed discovery remains saved.`);
				if (n.outcome === `no_unaddressed_${t === "outputs" ? "output" : "input"}`) break;
				if (n.outcome !== "assigned") throw Error("An address is reserved but not confirmed. Review the saved item before continuing.");
				await i("discover_address", {
					bus: e.bus,
					space: t,
					address: n.address
				});
			}
			for (let t of this.currentRows.filter((t) => t.bus.bus === e.bus && (t.kind === "output" || t.kind === "instance"))) {
				if (this.cancelled) break;
				if (!q(r, this.snapshot, n, e.bus)) throw Error("Configuration changed before refresh.");
				this.progress = `Reading ${t.item.name}`, this.touch(), await this.read(t);
			}
			this.notice = this.cancelled ? "Stopped after the current operation. Completed changes remain saved." : "Discovery and current-value refresh finished.";
		} finally {
			this.progress = "", this.touch();
		}
	}
	async createGroup(e, t, n) {
		let r = this.snapshot;
		await this.metadata("create_group", null, W(r.inventory.inventory_epoch, r.inventory.revision), {
			bus: e.bus,
			group: t,
			name: n
		});
	}
	async group(e, t, n) {
		let r = this.current(e.key), i = await this.metadata("assign_group", r.item.target, r.generation, {
			group: t,
			member: n
		});
		if (i.outcome !== "verified") throw Error("Saved assignment is not yet confirmed in hardware. Run Verify groups.");
		return i;
	}
	async readGroups(e) {
		let t = this.current(e.key);
		return this.client.job(t.generation, t.item.target, "read_groups", {});
	}
	async reviewGroups(e, t, n) {
		let r = this.current(e.key);
		return this.metadata(t, r.item.target, r.generation, { reviewed_mask: n });
	}
}, ze = 0;
Array.isArray;
function J(t, n, r, i, a, o) {
	n || (n = {});
	var s, c, l = n;
	if ("ref" in l && typeof t != "function") for (c in l = {}, n) c == "ref" ? s = n[c] : l[c] = n[c];
	var u = {
		type: t,
		props: l,
		key: r,
		ref: s,
		__k: null,
		__: null,
		__b: 0,
		__e: null,
		__c: null,
		constructor: void 0,
		__v: --ze,
		__i: -1,
		__u: 0
	};
	return (a || o) && (u.__source = a, u.__self = o), e.vnode && e.vnode(u), u;
}
// src/ui.tsx
var Be = L(null);
function Y() {
	let e = ye(Be);
	if (!e) throw Error("Controller is unavailable.");
	return e;
}
function Ve() {
	let e = Y();
	return e.progress ? /* @__PURE__ */ J("div", {
		class: "progress",
		role: "status",
		children: [/* @__PURE__ */ J("span", { children: e.progress }), /* @__PURE__ */ J("button", {
			onClick: () => {
				e.cancelled = !0, e.touch();
			},
			disabled: e.cancelled,
			children: e.cancelled ? "Stopping…" : "Stop after current step"
		})]
	}) : null;
}
function He(e) {
	history.pushState(null, "", e), dispatchEvent(new PopStateEvent("popstate"));
}
function X(e) {
	return /* @__PURE__ */ J("a", {
		href: e.href,
		class: e.class,
		onClick: (t) => {
			t.metaKey || t.ctrlKey || t.shiftKey || t.altKey || (t.preventDefault(), He(e.href));
		},
		children: e.children
	});
}
function Z(e) {
	let t = Y(), n = _e(null);
	return U(() => (n.current?.showModal(), () => n.current?.close()), []), /* @__PURE__ */ J("dialog", {
		ref: n,
		onCancel: (t) => {
			t.preventDefault(), e.close();
		},
		onClick: (t) => {
			t.target === t.currentTarget && e.close();
		},
		children: /* @__PURE__ */ J("div", {
			class: "dialog-body",
			children: [
				/* @__PURE__ */ J("header", { children: [/* @__PURE__ */ J("h2", { children: e.title }), /* @__PURE__ */ J("button", {
					type: "button",
					onClick: e.close,
					"aria-label": "Close dialog",
					children: "×"
				})] }),
				e.children,
				t.error && /* @__PURE__ */ J("p", {
					class: "error",
					role: "alert",
					children: t.error
				})
			]
		})
	});
}
function Ue({ row: e }) {
	let t = Y(), n = t.drafts.get(e.key), r = () => {
		let n = t.drafts.get(e.key);
		n && (n.commit((n, r) => t.rename(e.key, n, r)).catch(() => {}).finally(() => t.touch()), t.touch());
	}, i = _e(null);
	return /* @__PURE__ */ J("div", {
		class: "name-cell",
		children: [
			/* @__PURE__ */ J("div", {
				class: `name-edit ${n?.dirty ? "dirty" : ""}`,
				children: [/* @__PURE__ */ J("input", {
					ref: i,
					class: "name-field",
					"aria-label": `Name ${e.item.name}`,
					value: n?.text ?? e.item.name,
					disabled: !!n?.pending || !!t.progress,
					autoComplete: "off",
					autoCapitalize: "off",
					spellcheck: !1,
					onInput: (n) => {
						let r = t.drafts.get(e.key);
						r || (r = new Ne(e.item.name, e.item.target, e.generation), t.drafts.set(e.key, r)), r.text = n.currentTarget.value, r.error = "", t.touch();
					},
					onBlur: r,
					onKeyDown: (n) => {
						n.key === "Enter" && (n.preventDefault(), r()), n.key === "Escape" && (t.drafts.delete(e.key), t.touch(), i.current?.blur());
					}
				}), n?.dirty && /* @__PURE__ */ J("button", {
					type: "button",
					"aria-label": `Save name ${e.item.name}`,
					disabled: !!n.pending,
					onPointerDown: (e) => e.preventDefault(),
					onClick: r,
					children: "✓"
				})]
			}),
			n?.pending && /* @__PURE__ */ J("p", {
				class: "hint",
				role: "status",
				children: "Saving…"
			}),
			n?.error && /* @__PURE__ */ J("p", {
				class: "error",
				role: "alert",
				children: [
					n.error,
					" ",
					/* @__PURE__ */ J("button", {
						class: "text-button",
						onPointerDown: (e) => e.preventDefault(),
						onClick: () => {
							t.drafts.delete(e.key), t.touch();
						},
						children: "Discard draft"
					})
				]
			})
		]
	});
}
// src/button-timing.ts
var We = [
	"short",
	"double",
	"repeat",
	"stuck"
], Q = (e) => typeof e == "number" && Number.isInteger(e) && e >= 0 && e <= 255;
function Ge(e) {
	return Q(e.short) && Q(e.short_min) && e.short_min >= 10 && e.short >= e.short_min && Q(e.double) && Q(e.double_min) && e.double_min >= 10 && e.double_min <= 100 && (e.double === 0 || e.double >= e.double_min && e.double <= 100) && Q(e.repeat) && e.repeat >= 5 && e.repeat <= 100 && Q(e.stuck) && e.stuck >= 5;
}
function Ke(e) {
	if (e == null) return null;
	if (typeof e != "object" || Array.isArray(e) || !["complete", "partial"].includes(e.quality) || [
		...We,
		"short_min",
		"double_min"
	].some((t) => e[t] !== null && !Q(e[t])) || e.quality === "complete" && !Ge(e)) throw Error("Button timing evidence is incomplete or invalid. Inspect again.");
	return structuredClone(e);
}
function qe(e, t) {
	if (!t || t.quality !== "complete" || !Ge(t)) throw Error("Complete button timing evidence is required. Inspect again.");
	if (!Object.keys(e).length || Object.keys(e).some((e) => !We.includes(e))) throw Error("Change at least one known button timer.");
	if (!Ge({
		...t,
		...e
	})) throw Error("Button timing must use whole native steps within the range and device minimum.");
}
function Je(e, t) {
	return !!t && t.quality === "complete" && Ge(t) && We.every((n) => e[n] === void 0 || e[n] === t[n]);
}
var Ye = (e, t) => t * (e === "stuck" ? 1 : 20), Xe = (e, t) => t.trim() === "" ? NaN : Number(t) / (e === "stuck" ? 1 : 20);
function Ze(e, t) {
	let n = {};
	for (let r of We) e[r] !== t[r] && (n[r] = t[r] ?? NaN);
	return n;
}
// src/sensor-settings.ts
var Qe = [
	"report",
	"deadtime",
	"hold",
	"hysteresis",
	"hysteresis_min"
], $e = (e) => typeof e == "number" && Number.isInteger(e) && e >= 0 && e <= 255;
function et(e) {
	return e === 2 ? ["report", "deadtime"] : e === 3 ? [
		"report",
		"deadtime",
		"hold"
	] : e === 4 ? [
		"report",
		"deadtime",
		"hysteresis",
		"hysteresis_min"
	] : [];
}
function tt(e, t) {
	return $e(e.report) && $e(e.deadtime) && (t === 2 || t === 3 && $e(e.hold) && e.hold_supported === (e.hold !== 255) || t === 4 && $e(e.hysteresis) && e.hysteresis <= 25 && $e(e.hysteresis_min));
}
function nt(e, t) {
	if (e == null) return null;
	let n = et(t);
	if (!n.length || typeof e != "object" || Array.isArray(e) || !["complete", "partial"].includes(e.quality) || Qe.some((t) => e[t] !== null && !$e(e[t])) || Qe.some((t) => !n.includes(t) && e[t] !== null) || (t === 3 ? e.hold_supported !== (e.hold === null ? null : e.hold !== 255) : e.hold_supported !== null) || e.quality === "complete" && !tt(e, t)) throw Error("Sensor settings evidence is incomplete or invalid. Inspect again.");
	return structuredClone(e);
}
function rt(e, t, n) {
	if (!t || t.quality !== "complete" || !tt(t, n)) throw Error("Complete sensor settings evidence is required. Inspect again.");
	let r = Object.keys(e);
	if (!r.length || r.some((e) => !et(n).includes(e))) throw Error("Choose known settings for this sensor type.");
	if (e.hold !== void 0 && t.hold_supported !== !0) throw Error("This sensor does not support hold time.");
	if (r.some((t) => !$e(e[t]) || t === "hold" && e[t] === 255 || t === "hysteresis" && e[t] > 25)) throw Error("Sensor values must use valid native steps and ranges. Hold time is 1s or a 10s increment.");
}
function it(e, t, n) {
	return !!t && t.quality === "complete" && tt(t, n) && Qe.every((n) => e[n] === void 0 || e[n] === t[n] && (n !== "hold" || t.hold_supported === !0));
}
function at(e, t) {
	return e === "deadtime" ? t * 50 : e === "hold" ? t === 0 ? 1 : t * 10 : t;
}
function ot(e, t) {
	if (t.trim() === "") return NaN;
	let n = Number(t);
	return e === "deadtime" ? n / 50 : e === "hold" ? n === 1 ? 0 : n >= 10 ? n / 10 : NaN : n;
}
function st(e, t) {
	let n = {};
	for (let r of Qe) e[r] !== t[r] && (n[r] = t[r] ?? NaN);
	return n;
}
// src/input-settings.ts
var ct = {
	1: [
		"Released",
		"Pressed",
		"Short press",
		"Double press",
		"Long press start",
		"Long press repeat",
		"Long press stop",
		"Stuck / free"
	],
	2: ["Position changed"],
	3: [
		"Occupied",
		"Vacant",
		"Repeat",
		"Movement",
		"No movement"
	],
	4: ["Light level changed"]
}, lt = class {
	constructor(e, t) {
		if (this.cancelled = !1, this.values = null, t.kind !== "instance") throw Error("Select an input instance.");
		this.ui = e, this.row = structuredClone(t), this.initial = structuredClone(e.snapshot), this.epoch = e.client.views.epoch;
	}
	assert() {
		let e = this.ui, t = e.currentRows.find((e) => e.key === this.row.key);
		if (this.cancelled || e.cancelled || e.signingOut || !e.client.views.ready || e.client.views.epoch !== this.epoch) throw Error("Input settings stopped. Reopen to inspect current settings.");
		if (!q(this.initial, e.snapshot, this.initial.inventory.revision, this.row.bus.bus) || !K(e.snapshot, this.row.bus) || !t || t.kind !== "instance" || t.item.target !== this.row.item.target || t.generation !== this.row.generation || t.parent?.short_address !== this.row.parent?.short_address || t.item.number !== this.row.item.number || t.item.instance_type !== this.row.item.instance_type) throw Error("Input configuration changed. Reopen settings.");
	}
	accepted(e, t) {
		this.assert();
		let n = this.row;
		if (e.operation !== t || e.target !== n.item.target || e.target_generation !== n.generation || e.revision !== this.initial.inventory.revision || e.bus !== n.bus.bus || e.address !== n.parent?.short_address || e.instance !== n.item.number || e.reported_type !== n.item.instance_type || e.configuration_changed !== !1) throw Error("Input settings result changed. Inspect again.");
		if (e.outcome !== (t === "configure_input" ? "configured" : "current")) throw Error("Input settings are unavailable or unconfirmed. Inspect again.");
		let r = e.settings, i = {
			1: 255,
			2: 1,
			3: 31,
			4: 1
		}[n.item.instance_type ?? -1];
		if (!r || r.quality !== "complete" || typeof r.enabled != "boolean" || !Number.isInteger(r.event_priority) || r.event_priority < 2 || r.event_priority > 5 || !Number.isInteger(r.event_scheme) || r.event_scheme < 0 || r.event_scheme > 4 || !Number.isInteger(r.event_filter) || r.event_filter < 0 || r.event_filter > 16777215 || i !== void 0 && (r.event_filter & ~i) !== 0 || !Array.isArray(r.groups) || r.groups.length !== 3 || r.groups.some((e) => !Number.isInteger(e) || typeof e != "number" || e < 0 || e > 31 && e !== 255) || r.filter_width !== (i === void 0 ? 24 : 8)) throw Error("Input settings evidence is incomplete. Inspect again.");
		if (!Array.isArray(r.filter_bytes) || r.filter_bytes.length !== 3 || r.filter_bytes.some((e, t) => t < r.filter_width / 8 ? e !== (r.event_filter >>> 8 * t & 255) : e !== null)) throw Error("Input filter evidence is incomplete. Inspect again.");
		return {
			...structuredClone(r),
			button_timers: Ke(r.button_timers),
			sensor_settings: nt(r.sensor_settings, n.item.instance_type ?? -1)
		};
	}
	async read() {
		this.assert();
		let e = await this.ui.client.job(this.row.generation, this.row.item.target, "read_input_settings", {}), t = this.accepted(e, "read_input_settings");
		return this.values = t, structuredClone(t);
	}
	async save(e) {
		if (this.assert(), !this.values || !Object.keys(e).length) throw Error("Change at least one input setting.");
		let t = structuredClone(e);
		if (t.button_timers) {
			if (this.row.item.instance_type !== 1) throw Error("Button timing requires a button instance.");
			qe(t.button_timers, this.values.button_timers);
		}
		t.sensor_settings && rt(t.sensor_settings, this.values.sensor_settings, this.row.item.instance_type ?? -1);
		let n = await this.ui.client.job(this.row.generation, this.row.item.target, "configure_input", {
			expected_revision: this.initial.inventory.revision,
			settings: t
		}), r = this.accepted(n, "configure_input");
		for (let e of [
			"enabled",
			"event_priority",
			"event_scheme",
			"event_filter"
		]) if (t[e] !== void 0 && t[e] !== r[e]) throw Error("Input settings do not match the requested values. Inspect again.");
		if (t.groups && JSON.stringify(t.groups.map((e) => e ?? 255)) !== JSON.stringify(r.groups)) throw Error("Input groups do not match the requested values. Inspect again.");
		if (t.button_timers && !Je(t.button_timers, r.button_timers)) throw Error("Button timing does not match the requested values. Inspect again.");
		if (t.sensor_settings && !it(t.sensor_settings, r.sensor_settings, this.row.item.instance_type ?? -1)) throw Error("Sensor settings do not match the requested values. Inspect again.");
		return this.values = r, this.ui.notice = "Input event settings verified in hardware.", structuredClone(r);
	}
};
function ut(e, t) {
	let n = {};
	for (let r of [
		"enabled",
		"event_priority",
		"event_scheme",
		"event_filter"
	]) e[r] !== t[r] && Object.assign(n, { [r]: t[r] });
	if (JSON.stringify(e.groups) !== JSON.stringify(t.groups) && (n.groups = t.groups.map((e) => e === 255 ? null : e)), e.button_timers && t.button_timers) {
		let r = Ze(e.button_timers, t.button_timers);
		Object.keys(r).length && (n.button_timers = r);
	}
	if (e.sensor_settings && t.sensor_settings) {
		let r = st(e.sensor_settings, t.sensor_settings);
		Object.keys(r).length && (n.sensor_settings = r);
	}
	return n;
}
// src/input-settings-ui.tsx
function dt({ row: e, close: t }) {
	let n = Y(), [r] = H(() => new lt(n, e)), [i, a] = H(null), [o, s] = H("");
	ge(() => () => {
		r.cancelled = !0;
	}, [r]);
	let c = () => {
		r.cancelled = !0, t();
	}, l = () => void n.run(async () => {
		s(""), a(null);
		try {
			let e = await r.read();
			a(e);
		} catch (e) {
			throw r.cancelled || s(e.message), e;
		}
	});
	U(() => {
		l();
	}, []);
	let u = (e) => a(i ? {
		...i,
		...e
	} : null), d = () => void n.run(async () => {
		try {
			if (!i || !r.values) return;
			await r.save(ut(r.values, i)), c();
		} catch (e) {
			throw r.cancelled || s(e.message), e;
		}
	}), f = ct[e.item.instance_type ?? -1];
	return /* @__PURE__ */ J(Z, {
		title: `Event settings · ${e.item.name}`,
		close: c,
		children: [
			/* @__PURE__ */ J("p", {
				class: "hint",
				children: [
					r.initial.inventory.name,
					"/",
					e.bus.name,
					" · address ",
					e.parent?.short_address,
					" / instance ",
					e.item.number,
					". Changes are applied to this input's hardware."
				]
			}),
			o && /* @__PURE__ */ J("p", {
				class: "error",
				role: "alert",
				children: o
			}),
			!i && !o && /* @__PURE__ */ J("p", {
				role: "status",
				children: "Reading input settings…"
			}),
			i && /* @__PURE__ */ J("fieldset", {
				class: "input-settings-form",
				disabled: !!n.busy,
				children: [
					/* @__PURE__ */ J("label", { children: [/* @__PURE__ */ J("input", {
						type: "checkbox",
						checked: i.enabled,
						onChange: (e) => u({ enabled: e.currentTarget.checked })
					}), " Input enabled"] }),
					/* @__PURE__ */ J("label", { children: ["Event priority", /* @__PURE__ */ J("select", {
						"aria-label": "Event priority",
						value: i.event_priority,
						onChange: (e) => u({ event_priority: Number(e.currentTarget.value) }),
						children: [
							2,
							3,
							4,
							5
						].map((e) => /* @__PURE__ */ J("option", {
							value: e,
							children: [e, e === 2 ? " · highest" : e === 5 ? " · lowest" : ""]
						}))
					})] }),
					/* @__PURE__ */ J("label", { children: ["Event addressing", /* @__PURE__ */ J("select", {
						"aria-label": "Event addressing",
						value: i.event_scheme,
						onChange: (e) => u({ event_scheme: Number(e.currentTarget.value) }),
						children: [
							"Instance and type",
							"Device and type",
							"Device and instance",
							"Device group and type",
							"Instance group and type"
						].map((e, t) => /* @__PURE__ */ J("option", {
							value: t,
							children: e
						}))
					})] }),
					/* @__PURE__ */ J("p", {
						class: "hint",
						children: i.event_scheme === 2 ? "Device and instance identifies the source uniquely." : i.event_scheme === 1 ? "Device and type is routed only when exactly one matching instance is saved." : "This scheme is currently diagnostic only in the bridge. Use Device and instance for routed events."
					}),
					/* @__PURE__ */ J("fieldset", { children: [/* @__PURE__ */ J("legend", { children: "Events to send" }), f ? f.map((e, t) => /* @__PURE__ */ J("label", { children: [/* @__PURE__ */ J("input", {
						type: "checkbox",
						checked: !!(i.event_filter & 1 << t),
						onChange: (e) => u({ event_filter: e.currentTarget.checked ? i.event_filter | 1 << t : i.event_filter & ~(1 << t) })
					}), e] })) : /* @__PURE__ */ J("p", { children: [
						"Raw filter: ",
						i.event_filter.toString(16).toUpperCase().padStart(6, "0"),
						". This instance type has no known common event labels."
					] })] }),
					/* @__PURE__ */ J("fieldset", { children: [/* @__PURE__ */ J("legend", { children: "Input instance groups" }), [
						"Primary group",
						"Instance group 1",
						"Instance group 2"
					].map((e, t) => /* @__PURE__ */ J("label", { children: [e, /* @__PURE__ */ J("select", {
						"aria-label": e,
						value: i.groups[t],
						onChange: (e) => {
							let n = [...i.groups];
							n[t] = Number(e.currentTarget.value), u({ groups: n });
						},
						children: [/* @__PURE__ */ J("option", {
							value: "255",
							children: "Unassigned"
						}), Array.from({ length: 32 }, (e, t) => /* @__PURE__ */ J("option", {
							value: t,
							children: t
						}))]
					})] }))] }),
					e.item.instance_type === 1 && /* @__PURE__ */ J("fieldset", { children: [/* @__PURE__ */ J("legend", { children: "Button timing" }), r.values?.button_timers?.quality === "complete" && i.button_timers ? /* @__PURE__ */ J(y, { children: [/* @__PURE__ */ J("div", {
						class: "input-timer-grid",
						children: We.map((e) => {
							let t = i.button_timers, n = e === "short" ? t.short_min * 20 : e === "double" ? 0 : e === "repeat" ? 100 : 5, r = {
								short: "Short press (ms)",
								double: "Double press (ms)",
								repeat: "Long repeat (ms)",
								stuck: "Stuck button (s)"
							}[e], a = t[e];
							return /* @__PURE__ */ J("label", { children: [r, /* @__PURE__ */ J("input", {
								type: "number",
								inputMode: "numeric",
								"aria-label": r,
								min: n,
								max: e === "short" ? 5100 : e === "stuck" ? 255 : 2e3,
								step: e === "stuck" ? 1 : 20,
								value: Number.isFinite(a) ? Ye(e, a) : "",
								onChange: (n) => u({ button_timers: {
									...t,
									[e]: Xe(e, n.currentTarget.value)
								} })
							})] });
						})
					}), /* @__PURE__ */ J("p", {
						class: "hint",
						children: [
							"Short minimum: ",
							i.button_timers.short_min * 20,
							"ms. Double minimum: ",
							i.button_timers.double_min * 20,
							"ms; 0 disables double press. Other millisecond values use 20ms steps."
						]
					})] }) : /* @__PURE__ */ J("p", {
						class: "hint",
						children: "Button timing is unavailable or incomplete. Basic event settings remain available."
					})] }),
					et(e.item.instance_type ?? -1).length > 0 && /* @__PURE__ */ J("fieldset", { children: [/* @__PURE__ */ J("legend", { children: "Sensor settings" }), r.values?.sensor_settings?.quality === "complete" && i.sensor_settings ? /* @__PURE__ */ J(y, { children: [/* @__PURE__ */ J("div", {
						class: "input-timer-grid",
						children: et(e.item.instance_type).map((e) => {
							let t = i.sensor_settings;
							if (e === "hold" && !t.hold_supported) return /* @__PURE__ */ J("p", {
								class: "hint",
								children: "Hold time is not supported by this sensor."
							});
							let n = {
								report: "Report interval (s)",
								deadtime: "Deadtime (ms)",
								hold: "Hold time (s)",
								hysteresis: "Hysteresis (%)",
								hysteresis_min: "Minimum hysteresis (sensor units)"
							}[e], r = t[e];
							return /* @__PURE__ */ J("label", { children: [n, /* @__PURE__ */ J("input", {
								type: "number",
								inputMode: "numeric",
								"aria-label": n,
								min: +(e === "hold"),
								max: e === "deadtime" ? 12750 : e === "hold" ? 2540 : e === "hysteresis" ? 25 : 255,
								step: e === "deadtime" ? 50 : 1,
								value: Number.isFinite(r) ? at(e, r) : "",
								onChange: (n) => u({ sensor_settings: {
									...t,
									[e]: ot(e, n.currentTarget.value)
								} })
							})] });
						})
					}), /* @__PURE__ */ J("p", {
						class: "hint",
						children: [
							"0 disables reporting, deadtime or percentage hysteresis.",
							e.item.instance_type === 3 ? " Hold time: 1s or 10s increments." : "",
							e.item.instance_type === 4 ? " Minimum hysteresis uses sensor input units." : ""
						]
					})] }) : /* @__PURE__ */ J("p", {
						class: "hint",
						children: "Sensor settings are unavailable or incomplete. Basic event settings remain available."
					})] })
				]
			}),
			/* @__PURE__ */ J("div", {
				class: "dialog-actions",
				children: [
					/* @__PURE__ */ J("button", {
						onClick: c,
						children: "Cancel"
					}),
					/* @__PURE__ */ J("button", {
						disabled: !!n.busy,
						onClick: l,
						children: "Refresh settings"
					}),
					/* @__PURE__ */ J("button", {
						class: "primary",
						disabled: !!n.busy || !i,
						onClick: d,
						children: "Save settings"
					})
				]
			})
		]
	});
}
// src/feedback.ts
var ft = (e) => typeof e == "number" && Number.isInteger(e) && e >= 0 && e <= 255;
function pt(e) {
	let t = () => {
		throw Error("Feedback evidence is incomplete or invalid. Refresh to inspect again.");
	};
	if (!e || typeof e != "object" || Array.isArray(e) || !["complete", "partial"].includes(e.quality)) return t();
	let { features: n, capability: r, supported: i, active: a } = e;
	if (n !== null && (!Array.isArray(n) || n.length > 65 || n.some((e, t) => !Number.isInteger(e) || typeof e != "number" || e < 32 || e > 96 || t > 0 && n[t - 1] >= e)) || r !== null && !ft(r) || i !== null && typeof i != "boolean" || a !== null && typeof a != "boolean") return t();
	let o = null, s = "partial";
	if (n === null) {
		if (r !== null || a !== null) return t();
	} else if (!n.includes(32)) {
		if (o = !1, s = "complete", r !== null || a !== null) return t();
	} else if (r === null) {
		if (a !== null) return t();
	} else if (r === 0) {
		if (o = !1, s = "complete", a !== null) return t();
	} else if (r & 9) o = !0, typeof a == "boolean" && (s = "complete");
	else if (a !== null) return t();
	return i !== o || e.quality !== s ? t() : structuredClone(e);
}
function mt(e) {
	return !!e && e.quality === "complete" && e.supported === !0 && e.features?.includes(32) && typeof e.capability == "number" && !!(e.capability & 9) && typeof e.active == "boolean";
}
var ht = class {
	constructor(e, t) {
		if (this.cancelled = !1, this.values = null, t.kind !== "input" && t.kind !== "instance") throw Error("Select an input module or instance.");
		this.ui = e, this.row = structuredClone(t), this.initial = structuredClone(e.snapshot), this.epoch = e.client.views.epoch;
	}
	assert() {
		let { ui: e, row: t } = this, n = e.currentRows.find((e) => e.key === t.key);
		if (this.cancelled || e.cancelled || e.signingOut || !e.client.views.ready || e.client.views.epoch !== this.epoch) throw Error("Feedback stopped. Reopen to inspect again.");
		if (!q(this.initial, e.snapshot, this.initial.inventory.revision, t.bus.bus) || !K(e.snapshot, t.bus) || !n || n.kind !== t.kind || n.item.target !== t.item.target || n.generation !== t.generation || (t.kind === "input" ? n.item.short_address !== t.item.short_address : n.parent?.short_address !== t.parent?.short_address || n.item.number !== t.item.number || n.item.instance_type !== t.item.instance_type)) throw Error("Input configuration changed. Reopen feedback.");
	}
	accepted(e, t) {
		this.assert();
		let n = this.row, r = n.kind === "input" ? n.item.short_address : n.parent?.short_address, i = n.kind === "instance" ? n.item.number : null;
		if (e.operation !== t || e.target !== n.item.target || e.target_generation !== n.generation || e.revision !== this.initial.inventory.revision || e.bus !== n.bus.bus || e.address !== r || e.instance !== i || e.configuration_changed !== !1 || n.kind === "input" && e.reported_type !== null || n.kind === "instance" && e.feedback !== null && e.reported_type !== n.item.instance_type) throw Error("Feedback result changed. Refresh to inspect again.");
		let a = e.feedback === null ? null : pt(e.feedback);
		if (t === "set_input_feedback") {
			if (e.outcome !== "set" || !mt(a)) throw Error("Feedback was not confirmed. Refresh to inspect again.");
		} else if (e.outcome === "current") {
			if (!mt(a)) throw Error("Current feedback evidence is incomplete.");
		} else if (e.outcome === "unsupported") {
			if (!a || a.quality !== "complete" || a.supported !== !1) throw Error("Unsupported feedback evidence is incomplete.");
		} else if (e.outcome === "unavailable") {
			if (a && a.quality !== "partial") throw Error("Unavailable feedback evidence is inconsistent.");
		} else if (e.outcome !== "type_mismatch" || a !== null) throw Error("Feedback outcome is invalid.");
		return a;
	}
	async read() {
		this.assert(), this.values = null;
		let e = await this.ui.client.job(this.row.generation, this.row.item.target, "read_input_feedback", {}), t = this.accepted(e, "read_input_feedback");
		return this.values = t, structuredClone(t);
	}
	async set(e) {
		if (this.assert(), typeof e != "boolean" || !mt(this.values)) throw Error("Complete supported feedback evidence is required. Refresh first.");
		this.values = null;
		let t = await this.ui.client.job(this.row.generation, this.row.item.target, "set_input_feedback", {
			expected_revision: this.initial.inventory.revision,
			active: e
		}), n = this.accepted(t, "set_input_feedback");
		if (n?.active !== e) throw Error("Feedback does not match the requested state. Refresh to inspect again.");
		return this.values = n, this.ui.notice = "Feedback verified in hardware.", structuredClone(n);
	}
};
// src/feedback-ui.tsx
function gt({ row: e, close: t }) {
	let n = Y(), [r] = H(() => new ht(n, e)), [i, a] = H(null), [o, s] = H(!1), [c, l] = H("");
	ge(() => () => {
		r.cancelled = !0;
	}, [r]);
	let u = () => {
		r.cancelled = !0, t();
	}, d = () => void n.run(async () => {
		l(""), a(null), s(!1);
		try {
			let e = await r.read();
			a(e), s(!0);
		} catch (e) {
			throw r.cancelled || l(e.message), e;
		}
	});
	U(() => {
		d();
	}, []);
	let f = () => void n.run(async () => {
		let e = i?.active !== !0;
		l(""), a(null), s(!1);
		try {
			a(await r.set(e)), s(!0);
		} catch (e) {
			throw r.cancelled || (s(!0), l(e.message)), e;
		}
	}), p = i?.capability ?? 0;
	return /* @__PURE__ */ J(Z, {
		title: `Feedback · ${e.item.name}`,
		close: u,
		children: [
			/* @__PURE__ */ J("p", {
				class: "hint",
				children: [
					r.initial.inventory.name,
					"/",
					e.bus.name,
					" · address ",
					e.kind === "input" ? e.item.short_address : e.parent?.short_address,
					e.kind === "instance" ? ` / instance ${e.item.number}` : " · module"
				]
			}),
			c && /* @__PURE__ */ J("p", {
				class: "error",
				role: "alert",
				children: c
			}),
			!o && !c && /* @__PURE__ */ J("p", {
				role: "status",
				children: "Reading feedback…"
			}),
			o && (mt(i) ? /* @__PURE__ */ J("div", {
				class: "toolbar",
				children: [
					/* @__PURE__ */ J("span", { children: [
						p & 1 ? "Visible indicator" : "",
						(p & 9) == 9 ? " and " : "",
						p & 8 ? "Audible feedback" : ""
					] }),
					/* @__PURE__ */ J("span", { class: "spacer" }),
					/* @__PURE__ */ J("button", {
						class: `power ${i?.active ? "on" : "off"}`,
						"aria-label": "Feedback active",
						"aria-pressed": i?.active ?? !1,
						disabled: !!n.busy,
						onClick: f,
						children: i?.active ? "On" : "Off"
					})
				]
			}) : /* @__PURE__ */ J("p", { children: i?.supported === !1 ? "This input does not report supported feedback." : "Feedback is unavailable. Refresh to read it again." })),
			/* @__PURE__ */ J("div", {
				class: "dialog-actions",
				children: [/* @__PURE__ */ J("button", {
					onClick: u,
					children: "Close"
				}), /* @__PURE__ */ J("button", {
					disabled: !!n.busy,
					onClick: d,
					children: "Refresh feedback"
				})]
			})
		]
	});
}
// src/restoration.ts
var _t = class {
	constructor(e, t) {
		if (this.candidates = /* @__PURE__ */ new Set(), this.cancelled = !1, t.kind !== "output" && t.kind !== "input") throw Error("Select a saved fixture or input module.");
		this.ui = e, this.initial = structuredClone(e.snapshot), this.epoch = e.client.views.epoch, this.original = structuredClone(t), this.space = t.kind === "output" ? "outputs" : "inputs", this.revision = this.initial.inventory.revision, this.address = t.item.short_address, this.generation = t.generation;
		for (let n of e.currentRows) n.kind === t.kind && n.bus.bus === t.bus.bus && n.key !== t.key && this.candidates.add(n.key);
	}
	live() {
		if (this.cancelled || this.ui.cancelled || this.ui.signingOut || !this.ui.client.views.ready || this.ui.client.views.epoch !== this.epoch) throw Error("Address recovery stopped. Completed changes remain saved; inspect before retrying.");
	}
	assert() {
		this.live();
		let e = this.ui.snapshot, t = e.inventory.buses.find((e) => e.bus === this.original.bus.bus), n = G(e).find((e) => e.key === this.original.key);
		if (!q(this.initial, e, this.revision, this.original.bus.bus) || !t || !K(e, t) || !n || n.kind === "scene" || n.item.target !== this.original.item.target || n.generation !== this.generation || n.item.short_address !== this.address) throw Error("Configuration or saved fixture changed. Review before continuing address recovery.");
	}
	async accept(e, t) {
		if (this.live(), !Number.isSafeInteger(e) || Number(e) < this.revision) throw Error("Address recovery result changed. Inspect before retrying.");
		await this.ui.changed(Number(e)), this.revision = Number(e), t && (this.address = t.address, this.generation = t.generation), this.assert();
	}
	async controller(e, t) {
		this.assert();
		let n = await this.ui.client.job(W(this.initial.inventory.inventory_epoch, this.revision), null, e, {
			...t,
			bus: this.original.bus.bus,
			expected_revision: this.revision
		}), r = n.probe ?? n;
		if (r.bus !== this.original.bus.bus || r.space !== this.space) throw Error("Candidate discovery result changed. Inspect saved devices.");
		return await this.accept(n.revision), n;
	}
	async find() {
		let e = await this.controller(this.space === "outputs" ? "assign_next_output" : "assign_next_input", {});
		if (e.outcome === "no_free_address") throw Error("No free temporary address. Use the sole-unaddressed option after deliberately isolating the intended fixture.");
		if (e.outcome === `no_unaddressed_${this.space === "outputs" ? "output" : "input"}`) {
			this.ui.notice = "No unaddressed fixture found. Completed candidates remain on this list.";
			return;
		}
		if (e.outcome !== "assigned" || !Number.isInteger(e.address) || e.address < 0 || e.address > 63) throw Error("Temporary address is unconfirmed. Inspect the saved reservation before continuing.");
		let t = e.address;
		await this.controller("discover_address", {
			space: this.space,
			address: t
		});
		let n = this.ui.currentRows.find((e) => e.kind === this.original.kind && e.bus.bus === this.original.bus.bus && e.item.short_address === t);
		if (!n || n.key === this.original.key) throw Error("Temporary candidate changed. Inspect the saved reservation.");
		this.candidates.add(n.key);
		for (let e of this.ui.currentRows.filter((e) => e.bus.bus === n.bus.bus && (n.kind === "output" ? e.key === n.key : e.kind === "instance" && e.parent?.short_address === t))) this.assert(), await this.ui.read(e), this.assert();
		this.ui.notice = "Candidate added. Blink it or operate its inputs, then select Use this fixture on its row.";
	}
	review(e) {
		if (this.assert(), e) {
			let t = this.ui.currentRows.find((t) => t.key === e.key);
			if (!this.candidates.has(e.key) || !t || t.kind !== this.original.kind || t.bus.bus !== this.original.bus.bus || t.item.target !== e.item.target || t.generation !== e.generation) throw Error("Identified candidate changed. Review it again.");
		}
		return new vt(this, e);
	}
	async refresh() {
		try {
			await this.controller("discover_address", {
				space: this.space,
				address: this.address
			});
			for (let e of this.ui.currentRows.filter((e) => e.bus.bus === this.original.bus.bus && (this.space === "outputs" ? e.key === this.original.key : e.kind === "instance" && e.parent?.short_address === this.address))) this.assert(), await this.ui.read(e), this.assert();
			this.ui.notice = `Address restored at ${this.address}. Saved name and bindings preserved; current values refreshed.`;
		} catch {
			this.ui.notice = `Address restored at ${this.address}. Current-value refresh did not finish; use Refresh addressed or Read current values.`;
		}
	}
}, vt = class {
	constructor(e, t) {
		this.scope = e, this.revision = e.revision, this.candidate = t ? structuredClone(t) : void 0;
	}
	async run() {
		let e = this.scope, t = e.ui, n = e.original, r = this.candidate, i = () => {
			if (e.assert(), e.revision !== this.revision) throw Error("Recovery review changed. Review both fixtures again.");
			if (r) {
				let e = t.currentRows.find((e) => e.key === r.key);
				if (!e || e.item.target !== r.item.target || e.generation !== r.generation) throw Error("Identified candidate changed. Review it again.");
			}
		};
		i();
		let a = t.snapshot.identification;
		if (a && [
			"starting",
			"blinking",
			"restoring"
		].includes(a.phase)) {
			if (!r || a.target !== r.item.target || a.generation !== r.generation) throw Error("Stop the other blinking fixture before restoring an address.");
			if ((await t.client.job(r.generation, r.item.target, "stop_blink", {})).phase !== "restored") throw Error("Blink restoration is unconfirmed. Address recovery stopped.");
			i();
		}
		let o = r ? "restore_identified_address" : "restore_unaddressed_address", s = await t.client.job(n.generation, n.item.target, o, {
			expected_revision: this.revision,
			...r ? {
				candidate: r.item.target,
				candidate_generation: r.generation
			} : {}
		}), c = r?.item.short_address ?? n.item.short_address, l = r ? W(e.initial.inventory.inventory_epoch, s.revision) : n.generation;
		if (s.operation !== o || s.target !== n.item.target || s.target_generation !== n.generation || s.current_generation !== l || s.bus !== n.bus.bus || s.previous_address !== n.item.short_address || s.address !== c || r && (s.candidate !== r.item.target || s.candidate_generation !== r.generation || s.source_address !== r.item.short_address)) throw Error("Address restoration result changed. Inspect hardware before retrying.");
		if (await e.accept(s.revision, {
			address: c,
			generation: l
		}), s.outcome !== "restored") throw s.outcome === "no_unaddressed_fixture" ? Error("No unaddressed fixture found. No address was programmed.") : s.outcome === "multiple_unaddressed_fixtures" ? Error("Multiple unaddressed fixtures found. Identify candidates or isolate the intended fixture; no address was programmed.") : Error(s.outcome === "configuration_updated_but_unconfirmed" ? `Saved address updated to ${c}; the previous reservation is free, but current fixture evidence is unconfirmed. Refresh addressed devices before retrying.` : "Address restoration is unconfirmed. Inspect hardware before retrying.");
		if (r && t.currentRows.some((e) => e.key === r.key)) throw Error("Address restored but candidate retirement is not visible. Inspect saved configuration.");
		await e.refresh();
	}
}, yt = class {
	constructor(e, t) {
		this.scope = new _t(e, t);
	}
	async run() {
		let e = this.scope, t = e.ui, n = e.original;
		e.assert();
		let r = t.snapshot.identification;
		if (r && [
			"starting",
			"blinking",
			"restoring"
		].includes(r.phase)) {
			if (r.target !== n.item.target || r.generation !== n.generation) throw Error("Stop the other blinking fixture before clearing an address.");
			if ((await t.client.job(n.generation, n.item.target, "stop_blink", {})).phase !== "restored") throw Error("Blink restoration is unconfirmed. Address clearing stopped.");
			e.assert();
		}
		let i = await t.client.job(n.generation, n.item.target, "clear_short_address", { expected_revision: e.revision });
		if (i.operation !== "clear_short_address" || i.target !== n.item.target || i.target_generation !== n.generation || i.current_generation !== n.generation || i.bus !== n.bus.bus || i.previous_address !== n.item.short_address || i.address !== n.item.short_address || i.revision !== e.revision || i.configuration_changed !== !1) throw Error("Address clearing result changed. Inspect hardware before retrying.");
		if (await e.accept(i.revision), i.outcome !== "cleared") throw Error("Address clearing is unconfirmed. Inspect hardware before retrying.");
		t.notice = "Short address cleared. Saved name and bindings retained. Use Add unaddressed, then Restore lost address to identify it again.";
	}
};
// src/restoration-ui.tsx
function bt({ scope: e, review: t, choose: n, close: r }) {
	let i = Y(), a = e.original, o = i.snapshot.identification, s = !!t?.candidate && o?.target === t.candidate.item.target && [
		"starting",
		"blinking",
		"restoring"
	].includes(o.phase), c = (e) => {
		i.cancelled = !1, i.run(async () => {
			i.progress = "Recovering saved fixture", i.touch();
			try {
				await e();
			} finally {
				i.progress = "", i.touch();
			}
		});
	};
	return /* @__PURE__ */ J(y, { children: [/* @__PURE__ */ J("div", {
		class: "card recovery",
		role: "region",
		"aria-label": "Lost address recovery",
		children: [
			/* @__PURE__ */ J("h3", { children: [
				"Recover ",
				a.item.name,
				" · previous address ",
				a.item.short_address
			] }),
			/* @__PURE__ */ J("p", { children: "Keep this saved name, identity and bindings. Find and identify its new address on this list." }),
			/* @__PURE__ */ J("div", {
				class: "toolbar",
				children: [
					/* @__PURE__ */ J("button", {
						disabled: !!i.busy,
						onClick: () => c(() => e.find()),
						children: "Find another candidate"
					}),
					/* @__PURE__ */ J("button", {
						disabled: !!i.busy,
						onClick: () => {
							try {
								n(e.review());
							} catch (e) {
								i.error = e.message, i.touch();
							}
						},
						children: "Restore sole unaddressed fixture"
					}),
					/* @__PURE__ */ J("button", {
						onClick: () => {
							e.cancelled = !0, r();
						},
						children: "Close recovery"
					})
				]
			}),
			/* @__PURE__ */ J("p", {
				class: "hint",
				children: [
					"Candidates stay on this list if you close recovery. ",
					e.space === "outputs" ? "Blink a candidate to identify it." : "Operate a candidate’s inputs and watch its current state and actions.",
					" Then choose Use this fixture on its module row."
				]
			})
		]
	}), t && /* @__PURE__ */ J(Z, {
		title: "Confirm address restoration",
		close: () => n(null),
		children: [t.candidate ? /* @__PURE__ */ J(y, { children: [
			/* @__PURE__ */ J("p", { children: [
				"Keep ",
				/* @__PURE__ */ J("strong", { children: a.item.name }),
				" and adopt address ",
				/* @__PURE__ */ J("strong", { children: t.candidate.item.short_address }),
				", currently assigned to ",
				/* @__PURE__ */ J("strong", { children: t.candidate.item.name }),
				"."
			] }),
			/* @__PURE__ */ J("p", { children: [
				"Previous address ",
				/* @__PURE__ */ J("strong", { children: a.item.short_address }),
				" becomes available again. The original saved fixture and its bindings continue."
			] }),
			/* @__PURE__ */ J("p", { children: [
				"The selected row ",
				/* @__PURE__ */ J("strong", { children: t.candidate.item.name }),
				e.space === "inputs" ? " and its input instances" : "",
				" will be removed",
				e.space === "outputs" ? ", including any of its saved group or scene references" : "",
				"."
			] }),
			s && /* @__PURE__ */ J("p", {
				class: "hint",
				children: "The candidate’s prior light level must be restored before adopting its address."
			})
		] }) : /* @__PURE__ */ J(y, { children: [
			/* @__PURE__ */ J("p", { children: [
				"Restore ",
				/* @__PURE__ */ J("strong", { children: a.item.name }),
				" at its previous address ",
				/* @__PURE__ */ J("strong", { children: a.item.short_address }),
				"."
			] }),
			/* @__PURE__ */ J("p", { children: "I associate the sole unaddressed fixture on this bus with this saved name." }),
			/* @__PURE__ */ J("p", { children: "No address is programmed if none or multiple unaddressed fixtures are found. Use candidate identification, or deliberately isolate the intended fixture, if several lost their addresses." })
		] }), /* @__PURE__ */ J("div", {
			class: "dialog-actions",
			children: [/* @__PURE__ */ J("button", {
				onClick: () => n(null),
				children: "Cancel"
			}), /* @__PURE__ */ J("button", {
				class: "primary",
				disabled: !!i.busy,
				onClick: () => {
					let e = t;
					n(null), c(async () => {
						await e.run(), r();
					});
				},
				children: t.candidate ? s ? "Stop blink and adopt address" : "Adopt identified address" : "Restore saved address"
			})]
		})]
	})] });
}
// src/devices.tsx
function xt({ row: e, close: t }) {
	let n = Y(), r = n.physical(e.key), i = Ae(n.snapshot, r), a = r.item.color_representation ?? "", o = i.ColorCapabilities, s = o?.supported_representations, c = o?.temperature_limits, l = a === "temperature" ? ["ColorTemperature"] : a === "xy" ? ["ColorX", "ColorY"] : a === "rgbwaf" ? [
		"ColorRed",
		"ColorGreen",
		"ColorBlue",
		"ColorWhite",
		"ColorAmber",
		"ColorFree"
	] : [], [u, d] = H({}), f = (e) => ({
		ColorTemperature: "Temperature (K)",
		ColorX: "x",
		ColorY: "y",
		ColorRed: "Red (%)",
		ColorGreen: "Green (%)",
		ColorBlue: "Blue (%)",
		ColorWhite: "White (%)",
		ColorAmber: "Amber (%)",
		ColorFree: "Free colour (%)"
	})[e], p = (e, t) => typeof t == "number" ? (a === "rgbwaf" ? t * 100 : t).toString() : "";
	return /* @__PURE__ */ J(Z, {
		title: `Colour · ${r.item.name}`,
		close: t,
		children: [
			/* @__PURE__ */ J("label", {
				class: "field",
				children: ["Representation", /* @__PURE__ */ J("select", {
					"aria-label": "Colour representation",
					value: a,
					disabled: !!n.busy,
					onChange: (e) => {
						let i = e.currentTarget.value;
						n.run(async () => {
							await n.representation(r, i), t();
						});
					},
					children: [
						/* @__PURE__ */ J("option", {
							value: "",
							children: "None selected"
						}),
						/* @__PURE__ */ J("option", {
							value: "temperature",
							children: "White temperature"
						}),
						/* @__PURE__ */ J("option", {
							value: "xy",
							children: "CIE xy"
						}),
						/* @__PURE__ */ J("option", {
							value: "rgbwaf",
							children: "RGBWAF channels"
						})
					]
				})]
			}),
			/* @__PURE__ */ J("p", {
				class: "hint",
				children: "Choose the values this fixture uses. Selecting a representation does not change its physical operating mode."
			}),
			/* @__PURE__ */ J("p", {
				class: "hint",
				children: [
					"Observed support: ",
					s ? s.length ? s.join(", ") : "no supported colour representations" : "unknown",
					o?.rgbwaf_channel_count ? ` · ${o.rgbwaf_channel_count} RGBWAF channels` : "",
					c?.warmest_kelvin && c.coolest_kelvin ? ` · ${Math.round(c.warmest_kelvin)}–${Math.round(c.coolest_kelvin)} K` : ""
				]
			}),
			/* @__PURE__ */ J("div", {
				class: "colour-fields",
				children: l.map((e) => /* @__PURE__ */ J("label", {
					class: "field",
					children: [
						f(e),
						/* @__PURE__ */ J("input", {
							type: "number",
							step: a === "temperature" ? 1 : "any",
							min: +(a === "temperature"),
							max: a === "rgbwaf" ? 100 : a === "xy" ? 1 : void 0,
							"aria-label": f(e),
							value: u[e] ?? p(e, i[e]),
							placeholder: "Unknown",
							onInput: (t) => d({
								...u,
								[e]: t.currentTarget.value
							})
						}),
						/* @__PURE__ */ J("span", {
							class: "hint",
							children: ["Actual: ", p(e, i[e]) || "unknown"]
						})
					]
				}, e))
			}),
			/* @__PURE__ */ J("p", {
				class: "hint",
				children: i.ColorQuality === void 0 ? "Read the fixture to discover colour support." : `Colour status: ${i.ColorQuality}`
			}),
			/* @__PURE__ */ J("div", {
				class: "dialog-actions",
				children: [/* @__PURE__ */ J("button", {
					disabled: !!n.busy,
					onClick: () => void n.run(() => n.read(r)),
					children: "Read values"
				}), /* @__PURE__ */ J("button", {
					class: "primary",
					disabled: !!n.busy || !l.length,
					onClick: () => n.run(async () => {
						let e = {};
						for (let t of l) {
							let n = u[t];
							if (n !== void 0 && n !== "") {
								let r = Number(n);
								if (!Number.isFinite(r)) throw Error("Enter numeric colour values.");
								e[t] = a === "rgbwaf" ? r / 100 : r;
							}
						}
						if (!Object.keys(e).length) throw Error("Change at least one colour value.");
						await n.apply(r, e), t();
					}),
					children: "Apply colour"
				})]
			})
		]
	});
}
function St({ row: e, close: t }) {
	let n = Y(), r = n.physical(e.key);
	return /* @__PURE__ */ J(Z, {
		title: `Groups · ${r.item.name}`,
		close: t,
		children: [/* @__PURE__ */ J("p", {
			class: "hint",
			children: "Saved membership. Verify groups to compare with hardware."
		}), /* @__PURE__ */ J("div", {
			class: "checklist",
			children: [(r.bus.groups ?? []).map((e) => /* @__PURE__ */ J("label", { children: [
				/* @__PURE__ */ J("input", {
					type: "checkbox",
					checked: e.members?.includes(r.item.short_address) ?? !1,
					disabled: !!n.busy,
					onChange: (t) => {
						let i = t.currentTarget.checked;
						n.run(() => n.group(r, e.group_address, i));
					}
				}),
				e.name,
				" ",
				/* @__PURE__ */ J("span", {
					class: "hint",
					children: e.group_address
				})
			] }, e.group_address)), !r.bus.groups?.length && /* @__PURE__ */ J("p", { children: "No groups are saved. Open the Groups tab to create one." })]
		})]
	});
}
function Ct({ row: e, recover: t, select: n }) {
	let r = Y(), i = r.snapshot, a = Ae(i, e), [o, s] = H(null);
	ge(() => () => {
		o && (o.scope.cancelled = !0);
	}, [o]);
	let c = () => {
		o && (o.scope.cancelled = !0), s(null);
	}, [l, u] = H(!1), [d, f] = H(!1), [p, m] = H(!1), [h, g] = H(!1), [_, v] = H(!1), [b, x] = H(!1), [S, C] = H(null), w = K(i, e.bus) && r.client.views.ready, T = i.identification?.target === e.item.target ? i.identification : null, E = !!T && [
		"starting",
		"blinking",
		"restoring"
	].includes(T.phase), D = r.client.events.find((t) => t.target === e.item.target && t.generation === e.generation), O = r.hints.get(e.key), k = e.kind === "output" ? O?.length ? O.map((e) => `DT${e}`).join(", ") : "Output" : e.kind === "input" ? "Input module" : je(e.item.instance_type), A = Me(a), j = typeof a.Brightness == "number" ? Math.round(a.Brightness * 100) : null;
	return /* @__PURE__ */ J("div", {
		class: `device-row ${e.kind === "output" ? "output-grid" : "input-grid"} ${E ? "identifying" : ""} ${D && Date.now() - D.received < 5e3 ? "active-input" : ""}`,
		"data-device-key": e.key,
		children: [
			/* @__PURE__ */ J("span", {
				class: "address",
				children: e.parent ? `${e.parent.short_address} / ${e.item.number}` : e.item.short_address
			}),
			/* @__PURE__ */ J(Ue, { row: e }),
			/* @__PURE__ */ J("span", {
				class: "device-type",
				children: k
			}),
			e.kind === "output" ? /* @__PURE__ */ J(y, { children: [
				/* @__PURE__ */ J("button", {
					class: `power ${a.PowerState === !0 ? "on" : a.PowerState === !1 ? "off" : "unknown"}`,
					"aria-label": `Power ${e.item.name}`,
					"aria-pressed": typeof a.PowerState == "boolean" ? a.PowerState : "mixed",
					disabled: !w || !!r.busy || E,
					onClick: () => void r.run(() => r.apply(e, { PowerState: a.PowerState !== !0 })),
					children: a.PowerState === !0 ? "On" : a.PowerState === !1 ? "Off" : "?"
				}),
				/* @__PURE__ */ J("div", {
					class: "brightness",
					children: [/* @__PURE__ */ J("input", {
						type: "range",
						min: "0",
						max: "100",
						value: S ?? j ?? 0,
						"aria-label": `Brightness ${e.item.name}`,
						disabled: !w || !!r.busy || E || j === null,
						onInput: (e) => C(e.currentTarget.value),
						onChange: (t) => {
							let n = Number(t.currentTarget.value) / 100;
							r.run(async () => {
								try {
									await r.apply(e, { Brightness: n });
								} finally {
									C(null);
								}
							});
						}
					}), /* @__PURE__ */ J("output", { children: [S ?? j ?? "—", j === null ? "" : "%"] })]
				}),
				/* @__PURE__ */ J("button", {
					class: "colour-button",
					"aria-label": `Colour ${e.item.name}`,
					disabled: !w || !!r.busy || E,
					onClick: () => g(!0),
					children: e.item.color_representation === "temperature" && typeof a.ColorTemperature == "number" ? `${Math.round(a.ColorTemperature)} K` : e.item.color_representation === "xy" ? "xy controls" : e.item.color_representation === "rgbwaf" ? "RGB controls" : "Colour"
				}),
				/* @__PURE__ */ J("button", {
					class: E ? "primary blink" : "blink",
					"aria-label": `${E ? "Stop blink" : "Blink"} ${e.item.name}`,
					disabled: !w || !!r.busy || T?.phase === "restoring",
					onClick: () => void r.run(() => r.blink(e)),
					children: T?.phase === "restoring" ? "Restoring…" : E ? "Stop blink" : "Blink"
				})
			] }) : /* @__PURE__ */ J(y, { children: [/* @__PURE__ */ J("span", {
				class: `input-state ${a.Available === !0 ? "" : "muted"}`,
				children: e.kind === "input" ? `${e.item.instances?.length ?? 0} instances` : A
			}), /* @__PURE__ */ J("span", {
				class: "activity",
				children: D?.action ?? "No recent action"
			})] }),
			/* @__PURE__ */ J("button", {
				class: "menu-button",
				"aria-label": `Actions ${e.item.name}`,
				onClick: () => m(!0),
				children: "⋮"
			}),
			p && /* @__PURE__ */ J(Z, {
				title: e.item.name,
				close: () => m(!1),
				children: /* @__PURE__ */ J("div", {
					class: "menu-actions",
					children: [
						e.kind === "instance" && /* @__PURE__ */ J("button", {
							disabled: !w || !!r.busy,
							onClick: () => {
								r.cancelled = !1, m(!1), u(!0);
							},
							children: "Event settings"
						}),
						e.kind !== "output" && /* @__PURE__ */ J("button", {
							disabled: !w || !!r.busy,
							onClick: () => {
								r.cancelled = !1, m(!1), f(!0);
							},
							children: "Feedback"
						}),
						e.kind !== "input" && /* @__PURE__ */ J("button", {
							disabled: !w || !!r.busy,
							onClick: () => {
								m(!1), r.run(() => r.read(e));
							},
							children: "Read current values"
						}),
						e.kind === "output" && /* @__PURE__ */ J("button", {
							onClick: () => {
								m(!1), v(!0);
							},
							children: "Group membership"
						}),
						e.kind === "output" && /* @__PURE__ */ J("button", {
							onClick: () => {
								m(!1), g(!0);
							},
							children: "Colour representation and values"
						}),
						e.kind !== "instance" && /* @__PURE__ */ J("button", {
							disabled: !w || !!r.busy,
							onClick: () => {
								m(!1), t(e);
							},
							children: "Restore lost address"
						}),
						e.kind !== "instance" && /* @__PURE__ */ J("button", {
							class: "danger",
							disabled: !w || !!r.busy,
							onClick: () => {
								r.cancelled = !1, m(!1), s(new yt(r, e));
							},
							children: "Clear short address"
						}),
						e.kind !== "instance" && /* @__PURE__ */ J("button", {
							class: "danger",
							onClick: () => {
								m(!1), x(!0);
							},
							children: "Remove from list"
						})
					]
				})
			}),
			l && /* @__PURE__ */ J(dt, {
				row: e,
				close: () => u(!1)
			}),
			d && /* @__PURE__ */ J(gt, {
				row: e,
				close: () => f(!1)
			}),
			o && /* @__PURE__ */ J(Z, {
				title: "Clear short address?",
				close: c,
				children: [
					/* @__PURE__ */ J("p", { children: [
						"Clear address ",
						/* @__PURE__ */ J("strong", { children: o.scope.original.item.short_address }),
						" from ",
						/* @__PURE__ */ J("strong", { children: o.scope.original.item.name }),
						" on ",
						o.scope.initial.inventory.name,
						"/",
						o.scope.original.bus.name,
						"."
					] }),
					/* @__PURE__ */ J("p", { children: "The saved name, bindings and address reservation stay. The fixture will be unaddressed. Use Add unaddressed, then Restore lost address to identify it again." }),
					/* @__PURE__ */ J("div", {
						class: "dialog-actions",
						children: [/* @__PURE__ */ J("button", {
							onClick: c,
							children: "Cancel"
						}), /* @__PURE__ */ J("button", {
							class: "danger",
							disabled: !!r.busy,
							onClick: () => void r.run(async () => {
								await o.run(), s(null);
							}),
							children: E ? "Stop blink and clear address" : "Clear address"
						})]
					})
				]
			}),
			n && /* @__PURE__ */ J("div", {
				class: "row-error",
				children: /* @__PURE__ */ J("button", {
					disabled: !w || !!r.busy,
					onClick: n,
					children: "Use this fixture"
				})
			}),
			h && /* @__PURE__ */ J(xt, {
				row: e,
				close: () => g(!1)
			}),
			_ && /* @__PURE__ */ J(St, {
				row: e,
				close: () => v(!1)
			}),
			b && /* @__PURE__ */ J(Z, {
				title: `Remove ${e.item.name}?`,
				close: () => x(!1),
				children: [/* @__PURE__ */ J("p", { children: [
					"Remove this ",
					e.kind === "input" ? "module and all its instances" : "fixture",
					" from the saved list. Its hardware address stays unchanged and discovery can find it again."
				] }), /* @__PURE__ */ J("div", {
					class: "dialog-actions",
					children: [/* @__PURE__ */ J("button", {
						onClick: () => x(!1),
						children: "Cancel"
					}), /* @__PURE__ */ J("button", {
						class: "danger",
						disabled: !!r.busy,
						onClick: () => void r.run(async () => {
							await r.remove(e), x(!1);
						}),
						children: "Remove from list"
					})]
				})]
			}),
			T?.phase === "interrupted" && /* @__PURE__ */ J("p", {
				class: "hint row-error",
				children: "Identification interrupted. Restoration was not confirmed."
			})
		]
	});
}
function wt({ bus: e }) {
	let t = Y(), [n, r] = H(""), [i, a] = H(null), [o, s] = H(null);
	U(() => () => {
		i && (i.cancelled = !0);
	}, [i]);
	let c = (e) => {
		t.cancelled = !1, s(null), r(""), a(new _t(t, e));
	}, l = (e) => i?.candidates.has(e.key) ? () => {
		try {
			s(i.review(e));
		} catch (e) {
			t.error = e.message, t.touch();
		}
	} : void 0, u = t.currentRows.filter((t) => t.kind !== "scene" && t.bus.bus === e.bus), d = (e) => ke(n, e.item.name) || !!t.drafts.get(e.key)?.dirty || !!t.drafts.get(e.key)?.error, f = u.filter((e) => e.kind === "output" && d(e)), p = u.filter((e) => (e.kind === "input" || e.kind === "instance") && d(e)), m = K(t.snapshot, e) && t.client.views.ready, h = [...t.drafts].filter(([e, n]) => n.dirty && !t.currentRows.some((t) => t.key === e));
	return /* @__PURE__ */ J(y, { children: [
		/* @__PURE__ */ J("div", {
			class: "toolbar discovery",
			children: [
				/* @__PURE__ */ J("button", {
					class: "primary",
					disabled: !m || !!t.busy,
					onClick: () => void t.run(() => t.scan(e, !0)),
					children: "Add unaddressed"
				}),
				/* @__PURE__ */ J("button", {
					disabled: !m || !!t.busy,
					onClick: () => void t.run(() => t.scan(e, !1)),
					children: "Refresh addressed"
				}),
				/* @__PURE__ */ J("span", {
					class: "hint",
					children: "Existing names and addresses are preserved."
				})
			]
		}),
		/* @__PURE__ */ J(Ve, {}),
		i && /* @__PURE__ */ J(bt, {
			scope: i,
			review: o,
			choose: s,
			close: () => {
				s(null), a(null);
			}
		}),
		/* @__PURE__ */ J("div", {
			class: "toolbar filter",
			children: [
				/* @__PURE__ */ J("input", {
					type: "search",
					value: n,
					placeholder: "Filter names · e.g. Light*",
					"aria-label": "Filter device names",
					onInput: (e) => r(e.currentTarget.value)
				}),
				[
					"",
					"Light*",
					"Input*"
				].map((e) => /* @__PURE__ */ J("button", {
					class: n === e ? "soft" : "",
					onClick: () => r(e),
					children: e || "All"
				})),
				/* @__PURE__ */ J("span", { class: "spacer" }),
				/* @__PURE__ */ J("span", {
					class: "hint",
					children: [
						u.filter((e) => e.kind === "output").length,
						" outputs · ",
						u.filter((e) => e.kind === "input").length,
						" input modules"
					]
				})
			]
		}),
		/* @__PURE__ */ J("div", {
			class: "section-heading",
			children: [/* @__PURE__ */ J("h2", { children: "Outputs" }), /* @__PURE__ */ J("span", {
				class: "hint",
				children: "✓, Enter or leave the field to save"
			})]
		}),
		/* @__PURE__ */ J("div", {
			class: "device-table",
			children: [
				/* @__PURE__ */ J("div", {
					class: "table-heading output-grid",
					children: [
						/* @__PURE__ */ J("span", { children: "Address" }),
						/* @__PURE__ */ J("span", { children: "Name" }),
						/* @__PURE__ */ J("span", { children: "Type" }),
						/* @__PURE__ */ J("span", { children: "Power" }),
						/* @__PURE__ */ J("span", { children: "Brightness" }),
						/* @__PURE__ */ J("span", { children: "Colour" }),
						/* @__PURE__ */ J("span", { children: "Identify" }),
						/* @__PURE__ */ J("span", {})
					]
				}),
				f.map((e) => /* @__PURE__ */ J(Ct, {
					row: e,
					recover: c,
					select: l(e)
				}, e.key)),
				!f.length && /* @__PURE__ */ J("p", {
					class: "empty",
					children: n ? "No matching outputs." : "No outputs are saved. Start discovery to find the fixtures on this bus."
				})
			]
		}),
		/* @__PURE__ */ J("div", {
			class: "section-heading",
			children: [/* @__PURE__ */ J("h2", { children: "Inputs" }), /* @__PURE__ */ J("span", {
				class: "hint",
				children: "Operate a switch to see its current state and action."
			})]
		}),
		/* @__PURE__ */ J("div", {
			class: "device-table",
			children: [
				/* @__PURE__ */ J("div", {
					class: "table-heading input-grid",
					children: [
						/* @__PURE__ */ J("span", { children: "Addr / instance" }),
						/* @__PURE__ */ J("span", { children: "Name" }),
						/* @__PURE__ */ J("span", { children: "Type" }),
						/* @__PURE__ */ J("span", { children: "Current state" }),
						/* @__PURE__ */ J("span", { children: "Last action" }),
						/* @__PURE__ */ J("span", {})
					]
				}),
				p.map((e) => /* @__PURE__ */ J(Ct, {
					row: e,
					recover: c,
					select: l(e)
				}, e.key)),
				!p.length && /* @__PURE__ */ J("p", {
					class: "empty",
					children: n ? "No matching inputs." : "No input modules are saved."
				})
			]
		}),
		h.map(([e, n]) => /* @__PURE__ */ J("div", {
			class: "card retired",
			children: [
				/* @__PURE__ */ J("p", { children: ["Removed or replaced fixture · unsaved name: ", /* @__PURE__ */ J("strong", { children: n.text })] }),
				/* @__PURE__ */ J("p", {
					class: "error",
					children: "This draft cannot be applied to a replacement."
				}),
				/* @__PURE__ */ J("button", {
					onClick: () => {
						t.drafts.delete(e), t.touch();
					},
					children: "Discard retired draft"
				})
			]
		}, e)),
		!m && /* @__PURE__ */ J("p", {
			class: "hint",
			children: e.enabled ? "Hardware controls are unavailable until the bus is ready." : "Enable this bus to use hardware controls."
		})
	] });
}
// src/group-output.ts
var $ = () => {
	throw Error("Group evidence is incomplete or inconsistent. Refresh to inspect again.");
}, Tt = (e) => typeof e == "number" && Number.isInteger(e) && e >= 0 && e <= 255;
function Et(e, t = !1) {
	return e === null ? null : !e || typeof e != "object" || Array.isArray(e) ? $() : e.kind === "value" ? !Tt(e.raw) || t && e.raw === 255 ? $() : e.raw : e.kind === "mask" ? !t || e.raw !== 255 ? $() : null : e.kind === "no_response" || ["receive_collision", "transmit_collision"].includes(e.kind) && (e.code === null || Tt(e.code)) ? null : $();
}
function Dt(e) {
	if (!e || typeof e != "object" || Array.isArray(e) || Object.keys(e).some((e) => e !== "power" && e !== "brightness") || !Object.keys(e).length) throw Error("Specify group power or a level.");
	if ("power" in e && typeof e.power != "boolean" || "brightness" in e && (typeof e.brightness != "number" || !Number.isFinite(e.brightness) || e.brightness < 0 || e.brightness > 1)) throw Error("Group power must be boolean and level must be between 0 and 1.");
	if (e.power === !1 && typeof e.brightness == "number" && e.brightness > 0 || e.power === !0 && e.brightness === 0) throw Error("Group power and level contradict each other.");
	return structuredClone(e);
}
var Ot = class {
	constructor(e, t) {
		if (this.cancelled = !1, this.result = null, t.kind !== "group") throw Error("Select a saved hardware group.");
		this.ui = e, this.row = structuredClone(t), this.initial = structuredClone(e.snapshot), this.epoch = e.client.views.epoch;
	}
	assert() {
		let { ui: e, row: t } = this, n = e.currentRows.find((e) => e.key === t.key);
		if (this.cancelled || e.cancelled || e.signingOut || !e.client.views.ready || e.client.views.epoch !== this.epoch) throw Error("Group control stopped. Refresh to inspect again.");
		if (!q(this.initial, e.snapshot, this.initial.inventory.revision, t.bus.bus) || !K(e.snapshot, t.bus) || !n || n.kind !== "group" || n.item.target !== t.item.target || n.generation !== t.generation || n.item.group_address !== t.item.group_address) throw Error("Group configuration changed. Refresh to inspect again.");
	}
	current() {
		try {
			return this.assert(), !0;
		} catch {
			return !1;
		}
	}
	accepted(e, t) {
		this.assert();
		let { row: n, initial: r } = this;
		if (!e || e.operation !== t || e.outcome !== (t === "read_group_output" ? "observed" : "sent") || e.target !== n.item.target || e.target_generation !== n.generation || e.revision !== r.inventory.revision || e.bus !== n.bus.bus || e.group !== n.item.group_address || e.coverage !== "saved_inventory" || e.configuration_changed !== !1) return $();
		let i = this.initial.inventory.buses.find((e) => e.bus === n.bus.bus).outputs;
		if (!Array.isArray(e.members) || e.members.length !== i.length || !e.values) return $();
		let a = /* @__PURE__ */ new Set(), o = [], s = [], c = !1, l = 0, u = !0, d = !0;
		for (let t of e.members) {
			let e = i.find((e) => e.short_address === t.address);
			if (!e || a.has(t.address) || t.target !== e.target || t.target_generation !== `${r.inventory.inventory_epoch}.${e.binding_revision}` || t.expected_member !== (n.item.members?.includes(t.address) ?? !1)) return $();
			a.add(t.address);
			let f = Et(t.membership_low), p = Et(t.membership_high), m = Et(t.actual_level, !0), h = Et(t.status);
			c || (c = f !== null || p !== null || m !== null || h !== null || t.actual_level?.kind === "mask");
			let g = f === null || p === null ? null : !!((f | p << 8) & 1 << n.item.group_address);
			if (t.member !== g) return $();
			u && (u = g !== null), d && (d = g === t.expected_member), g === null ? s.push(t.address) : g && (l++, m === null || h === null ? s.push(t.address) : o.push(m));
		}
		let f, p = null, m = null, h = null, g = null;
		s.length ? f = c ? "partial" : "unavailable" : o.length ? (h = o.some((e) => e > 0 != o[0] > 0), g = o.some((e) => e !== o[0]), f = g ? "mixed" : "uniform", h || (p = o[0] > 0), g || (m = o[0] / 254)) : f = "empty";
		let _ = e.values, v = {
			quality: f,
			power_state: p,
			brightness: m,
			power_mixed: h,
			brightness_mixed: g,
			known_members: l,
			unresolved: s
		};
		return Object.entries(v).some(([e, t]) => JSON.stringify(_[e]) !== JSON.stringify(t)) || e.membership_matches !== (u ? d : null) ? $() : {
			values: structuredClone(v),
			membership_matches: e.membership_matches
		};
	}
	async read() {
		this.assert(), this.result = null;
		let e = await this.ui.client.job(this.row.generation, this.row.item.target, "read_group_output", {});
		return this.result = this.accepted(e, "read_group_output"), structuredClone(this.result);
	}
	async set(e) {
		this.assert();
		let t = Dt(e);
		this.result = null;
		let n = await this.ui.client.job(this.row.generation, this.row.item.target, "set_group_output", {
			expected_revision: this.initial.inventory.revision,
			...t
		});
		return this.result = this.accepted(n, "set_group_output"), this.ui.notice = "Group command sent. Shown values are sampled from saved fixtures.", structuredClone(this.result);
	}
};
// src/group-output-ui.tsx
function kt({ row: e }) {
	let t = Y(), n = _e(null), [r, i] = H(null), [a, o] = H(50);
	ge(() => () => {
		n.current && (n.current.cancelled = !0);
	}, []);
	let s = (r) => void t.run(async () => {
		n.current && (n.current.cancelled = !0), t.cancelled = !1;
		let a = new Ot(t, e);
		n.current = a, i(null);
		let o = r ? await a.set(r) : await a.read();
		i(o);
	}), c = n.current?.current() ? r : null, l = c?.values, u = K(t.snapshot, e.bus) && t.client.views.ready && !t.busy, d = typeof l?.power_state == "boolean" ? l.power_state : null;
	return /* @__PURE__ */ J("section", {
		class: "card",
		"aria-label": "Group power and dimming",
		children: [
			/* @__PURE__ */ J("div", {
				class: "toolbar",
				children: [
					/* @__PURE__ */ J("strong", { children: "Power" }),
					d === null ? /* @__PURE__ */ J(y, { children: [
						/* @__PURE__ */ J("span", {
							class: "muted",
							children: l?.power_mixed ? "Mixed" : "Unknown"
						}),
						/* @__PURE__ */ J("button", {
							"aria-label": "Group on",
							disabled: !u,
							onClick: () => s({ power: !0 }),
							children: "On"
						}),
						/* @__PURE__ */ J("button", {
							"aria-label": "Group off",
							disabled: !u,
							onClick: () => s({ power: !1 }),
							children: "Off"
						})
					] }) : /* @__PURE__ */ J("button", {
						class: `power ${d ? "on" : "off"}`,
						"aria-label": "Group power",
						"aria-pressed": d,
						disabled: !u,
						onClick: () => s({ power: !d }),
						children: d ? "On" : "Off"
					}),
					/* @__PURE__ */ J("span", { class: "spacer" }),
					/* @__PURE__ */ J("button", {
						disabled: !u,
						onClick: () => s(),
						children: "Read group values"
					})
				]
			}),
			/* @__PURE__ */ J("div", {
				class: "toolbar",
				children: [
					/* @__PURE__ */ J("span", { children: ["Brightness: ", typeof l?.brightness == "number" ? `${Math.round(l.brightness * 100)}%` : l?.brightness_mixed ? "Mixed" : "Unknown"] }),
					/* @__PURE__ */ J("label", { children: ["New level ", /* @__PURE__ */ J("input", {
						type: "range",
						"aria-label": "Group level command",
						min: "0",
						max: "100",
						step: "1",
						value: a,
						disabled: !u,
						onInput: (e) => o(Number(e.currentTarget.value))
					})] }),
					/* @__PURE__ */ J("span", { children: [a, "%"] }),
					/* @__PURE__ */ J("button", {
						disabled: !u,
						onClick: () => s({ brightness: a / 100 }),
						children: "Set group level"
					})
				]
			}),
			/* @__PURE__ */ J("p", {
				class: "hint",
				children: [
					"Commands use hardware group ",
					e.item.group_address,
					". Readings cover saved fixtures.",
					c?.membership_matches === !1 ? " Assignments differ; use Verify all groups to review." : "",
					l && ["partial", "unavailable"].includes(l.quality) ? " Some fixtures could not be read." : "",
					l?.quality === "empty" ? " No saved fixture currently reports membership." : ""
				]
			})
		]
	});
}
// src/groups.tsx
function At({ bus: e }) {
	let t = Y(), [n, r] = H(0), [i, a] = H(""), [o, s] = H("add"), [c, l] = H(null), [u, d] = H(null), [f, p] = H(""), m = e.groups?.find((e) => e.group_address === n), h = t.currentRows.filter((t) => t.kind === "output" && t.bus.bus === e.bus), g = t.currentRows.find((t) => t.kind === "group" && t.bus.bus === e.bus && t.item.group_address === n), _ = K(t.snapshot, e) && t.client.views.ready;
	function v() {
		let e = h.map((e) => ({
			key: e.key,
			generation: e.generation,
			name: e.item.name,
			before: m?.members?.includes(e.item.short_address) ?? !1,
			after: o === "add" && (m?.members?.includes(e.item.short_address) ?? !1) || ke(i, e.item.name)
		})).filter((e) => e.before !== e.after);
		l({
			changes: e,
			initial: t.snapshot,
			revision: t.snapshot.inventory.revision,
			groupKey: g?.key,
			slot: n
		});
	}
	async function b() {
		let n = c;
		if (!n) return;
		l(null);
		let r = n.revision;
		t.cancelled = !1;
		try {
			for (let i of n.changes) {
				if (t.cancelled) break;
				if (t.progress = `Assigning group ${n.slot} · ${i.name}`, t.touch(), !q(n.initial, t.snapshot, r, e.bus) || !t.currentRows.some((e) => e.key === n.groupKey)) throw Error("Group configuration changed after preview. Review the pattern again.");
				let a = t.current(i.key);
				if (a.generation !== i.generation) throw Error("A selected fixture changed after preview. Review again.");
				r = (await t.group(a, n.slot, i.after)).revision;
			}
			t.notice = t.cancelled ? "Stopped after the current assignment. Completed changes remain saved." : "Pattern assignments verified.";
		} finally {
			t.progress = "", t.touch();
		}
	}
	async function x() {
		let n = t.snapshot, r = [];
		t.cancelled = !1;
		try {
			for (let i of h) {
				if (t.cancelled) break;
				if (!q(n, t.snapshot, n.inventory.revision, e.bus) || t.current(i.key).generation !== i.generation) throw Error("Configuration changed during verification. Start verification again.");
				t.progress = `Verifying groups · ${i.item.name}`, t.touch();
				let a = await t.readGroups(i);
				r.push({
					key: i.key,
					generation: a.target_generation,
					report: a,
					revision: n.inventory.revision
				});
			}
			d(r), t.notice = t.cancelled ? "Verification stopped. Results include completed reads only." : "Group verification finished. Review differences and unresolved fixtures.";
		} finally {
			t.progress = "", t.touch();
		}
	}
	async function S(e, n) {
		let r = t.current(e.key);
		if (r.generation !== e.generation || t.snapshot.inventory.revision !== e.revision) throw Error("Configuration changed since verification. Verify groups again.");
		if ((await t.reviewGroups(r, n, e.report.probe.mask)).outcome === "unconfirmed") throw Error("Hardware result remains unconfirmed. Verify again.");
		let i = await t.readGroups(t.current(e.key));
		d((n) => n?.map((n) => n.key === e.key ? {
			...n,
			report: i,
			revision: t.snapshot.inventory.revision
		} : n) ?? null), t.notice = n === "adopt_groups" ? "Hardware membership adopted." : "Saved membership repaired and re-read.";
	}
	return /* @__PURE__ */ J(y, { children: [/* @__PURE__ */ J(Ve, {}), /* @__PURE__ */ J("div", {
		class: "groups-layout",
		children: [/* @__PURE__ */ J("aside", {
			class: "slots",
			children: [/* @__PURE__ */ J("h2", { children: "Groups" }), Array.from({ length: 16 }, (t, i) => {
				let a = e.groups?.find((e) => e.group_address === i);
				return /* @__PURE__ */ J("button", {
					class: n === i ? "selected" : "",
					onClick: () => {
						r(i), p("");
					},
					children: [/* @__PURE__ */ J("span", { children: i }), a?.name ?? "Unused slot"]
				});
			})]
		}), /* @__PURE__ */ J("section", { children: [
			/* @__PURE__ */ J("div", {
				class: "section-heading",
				children: [/* @__PURE__ */ J("h2", { children: ["Group ", n] }), /* @__PURE__ */ J("button", {
					disabled: !_ || !!t.busy,
					onClick: () => void t.run(x),
					children: "Verify all groups"
				})]
			}),
			m && g ? /* @__PURE__ */ J(y, { children: [
				/* @__PURE__ */ J("div", {
					class: "card group-title",
					children: [/* @__PURE__ */ J(Ue, { row: g }), /* @__PURE__ */ J("span", {
						class: "hint",
						children: [
							m.members?.length ?? 0,
							" saved members · slot ",
							n
						]
					})]
				}),
				/* @__PURE__ */ J(kt, { row: g }, g.key + ":" + g.generation),
				/* @__PURE__ */ J("div", {
					class: "toolbar pattern",
					children: [
						/* @__PURE__ */ J("input", {
							"aria-label": "Group name pattern",
							value: i,
							placeholder: "Name pattern · e.g. Kitchen*",
							onInput: (e) => a(e.currentTarget.value)
						}),
						/* @__PURE__ */ J("select", {
							"aria-label": "Pattern assignment mode",
							value: o,
							onChange: (e) => s(e.currentTarget.value),
							children: [/* @__PURE__ */ J("option", {
								value: "add",
								children: "Add matching fixtures"
							}), /* @__PURE__ */ J("option", {
								value: "replace",
								children: "Replace this group's members"
							})]
						}),
						/* @__PURE__ */ J("button", {
							disabled: !i || !_ || !!t.busy,
							onClick: v,
							children: "Preview pattern"
						})
					]
				}),
				/* @__PURE__ */ J("p", {
					class: "hint",
					children: "Checkboxes show saved membership. Verification compares all 16 hardware group bits."
				}),
				/* @__PURE__ */ J("div", {
					class: "device-table membership-table",
					children: h.map((e) => /* @__PURE__ */ J("label", {
						class: "membership-row",
						children: [
							/* @__PURE__ */ J("input", {
								type: "checkbox",
								"aria-label": `Assign ${e.item.name} to ${m.name}`,
								checked: m.members?.includes(e.item.short_address) ?? !1,
								disabled: !_ || !!t.busy,
								onChange: (r) => {
									let i = r.currentTarget.checked;
									t.run(() => t.group(e, n, i).then(() => {}));
								}
							}),
							/* @__PURE__ */ J("span", {
								class: "address",
								children: e.item.short_address
							}),
							/* @__PURE__ */ J("span", { children: e.item.name }),
							/* @__PURE__ */ J("button", {
								type: "button",
								disabled: !_ || !!t.busy,
								onClick: (n) => {
									n.preventDefault(), t.run(() => t.blink(e));
								},
								children: "Blink"
							})
						]
					}, e.key))
				})
			] }) : /* @__PURE__ */ J("div", {
				class: "card",
				children: [
					/* @__PURE__ */ J("h3", { children: ["Unused group slot ", n] }),
					/* @__PURE__ */ J("p", {
						class: "muted",
						children: "Save a group name before assigning fixtures. Existing hardware membership is adopted only after review."
					}),
					/* @__PURE__ */ J("div", {
						class: "toolbar",
						children: [/* @__PURE__ */ J("input", {
							"aria-label": "New group name",
							placeholder: `Group${n + 1}`,
							value: f,
							onInput: (e) => p(e.currentTarget.value)
						}), /* @__PURE__ */ J("button", {
							class: "primary",
							disabled: !!t.busy,
							onClick: () => void t.run(() => t.createGroup(e, n, f || `Group${n + 1}`)),
							children: "Create group"
						})]
					})
				]
			}),
			c && /* @__PURE__ */ J(Z, {
				title: "Review pattern assignment",
				close: () => l(null),
				children: [
					/* @__PURE__ */ J("p", { children: [
						c.changes.length,
						" changes in group ",
						c.slot,
						". Other groups stay unchanged."
					] }),
					/* @__PURE__ */ J("ul", { children: c.changes.map((e) => /* @__PURE__ */ J("li", { children: [
						e.after ? "Add" : "Remove",
						" ",
						e.name
					] }, e.key)) }),
					!c.changes.length && /* @__PURE__ */ J("p", { children: "No membership changes are needed." }),
					/* @__PURE__ */ J("div", {
						class: "dialog-actions",
						children: [/* @__PURE__ */ J("button", {
							onClick: () => l(null),
							children: "Cancel"
						}), /* @__PURE__ */ J("button", {
							class: "primary",
							disabled: !c.changes.length || !!t.busy,
							onClick: () => void t.run(b),
							children: "Apply reviewed changes"
						})]
					})
				]
			}),
			u && /* @__PURE__ */ J(Z, {
				title: "Verify group assignments",
				close: () => d(null),
				children: [/* @__PURE__ */ J("p", {
					class: "hint",
					children: "Compare every saved fixture's complete hardware mask, including unnamed group slots. Missing replies stay unresolved."
				}), u.map((e) => {
					let n = t.currentRows.find((t) => t.key === e.key), r = e.report, i = r.probe?.quality === "complete" && !r.probe?.stale, a = i && r.probe.mask !== r.expected_mask;
					return /* @__PURE__ */ J("section", {
						class: "verify-row",
						children: [
							/* @__PURE__ */ J("h3", { children: n?.item.name ?? "Removed fixture" }),
							/* @__PURE__ */ J("p", { children: i ? a ? `Different · saved ${r.expected_mask.toString(16).padStart(4, "0")} · hardware ${r.probe.mask.toString(16).padStart(4, "0")}` : "Assignments match" : "Unresolved hardware reply" }),
							a && n && /* @__PURE__ */ J("div", {
								class: "dialog-actions",
								children: [/* @__PURE__ */ J("button", {
									disabled: !!t.busy,
									onClick: () => void t.run(() => S(e, "adopt_groups")),
									children: "Adopt hardware assignments"
								}), /* @__PURE__ */ J("button", {
									class: "primary",
									disabled: !!t.busy,
									onClick: () => void t.run(() => S(e, "repair_groups")),
									children: "Repair to saved assignments"
								})]
							})
						]
					}, e.key);
				})]
			})
		] })]
	})] });
}
// src/scene-settings.ts
var jt = () => ({
	level: { kind: "mask" },
	color: { kind: "mask" }
}), Mt = (e, t, n) => typeof e == "number" && Number.isInteger(e) && e >= t && e <= n;
function Nt(e) {
	if (!e || !e.level || !e.color || e.level.kind !== "mask" && !(e.level.kind === "value" && Mt(e.level.value, 0, 254))) return !1;
	let t = e.color, n = (e, t, n) => e === null || Mt(e, t, n);
	return t.kind === "mask" || t.kind === "temperature" && n(t.value, 1, 65534) || t.kind === "xy" && n(t.x, 0, 65534) && n(t.y, 0, 65534) || t.kind === "rgbwaf" && Array.isArray(t.channels) && t.channels.length === 6 && t.channels.every((e) => n(e, 0, 254)) && n(t.control, 0, 191);
}
function Pt(e) {
	if (!e) return "Unmanaged";
	let t = e.level.kind === "mask" ? "Keep brightness" : e.level.value === 0 ? "Off" : `${+(e.level.value * 100 / 254).toFixed(1)}%`, n = e.color;
	return n.kind === "mask" ? e.level.kind === "mask" ? "Leave unchanged" : t : `${t} · ${n.kind === "temperature" ? n.value === null ? "Tc MASK" : `${Math.round(1e6 / n.value)} K` : n.kind === "xy" ? "CIE xy" : "RGBWAF"}`;
}
var Ft = class {
	constructor(e) {
		this.fields = {}, this.dirty = !1, this.error = "", this.original = structuredClone(e);
		let t = e ?? jt();
		this.action = t.level.kind === "mask" ? t.color.kind === "mask" ? "ignore" : "keep" : t.level.value === 0 ? "off" : "brightness", this.family = t.color.kind;
	}
	chooseAction(e) {
		this.action = e, this.dirty = !0, this.error = "";
	}
	chooseFamily(e) {
		this.family = e, this.action === "ignore" && e !== "mask" && (this.action = "keep"), this.fields = { ...this.fields }, this.dirty = !0, this.error = "";
	}
	change(e, t) {
		this.fields[e] = t, this.dirty = !0, this.error = "";
	}
	text(e) {
		if (e in this.fields) return this.fields[e];
		let t = this.original ?? jt(), n = t.color;
		if (e === "brightness") return t.level.kind === "value" ? String(+(t.level.value * 100 / 254).toFixed(3)) : "100";
		if (e === "temperature") return n.kind === "temperature" && n.value !== null ? String(+(1e6 / n.value).toFixed(3)) : "";
		if (e === "x" || e === "y") return n.kind === "xy" && n[e] !== null ? String(n[e] / 65536) : "";
		if (e === "control") return n.kind === "rgbwaf" && n.control !== null ? String(n.control) : "";
		let r = It.indexOf(e);
		return r >= 0 && n.kind === "rgbwaf" && n.channels[r] !== null ? String(+(n.channels[r] * 100 / 254).toFixed(3)) : "";
	}
	setting() {
		let e = this.original ?? jt(), t = (e, t, n, r, i) => {
			if (!(e in this.fields)) return i;
			let a = this.fields[e].trim();
			if (a === "") return null;
			let o = Number(a);
			if (!Number.isFinite(o) || o < t || o > n) throw Error(`Enter ${e} between ${t} and ${n}, or leave it blank for MASK.`);
			return r(o);
		};
		if (this.action === "ignore") return jt();
		let n = { kind: "mask" };
		if (this.action === "off" && (n = {
			kind: "value",
			value: 0
		}), this.action === "brightness") {
			let r = t("brightness", 0, 100, (e) => Math.round(e * 254 / 100), e.level.kind === "value" ? e.level.value : 254);
			if (r === null) throw Error("Brightness is required; choose Keep brightness for MASK.");
			n = {
				kind: "value",
				value: r
			};
		}
		let r = { kind: "mask" }, i = e.color;
		this.family === "temperature" && (r = {
			kind: "temperature",
			value: t("temperature", 1e6 / 65534, 1e6, (e) => Math.round(1e6 / e), i.kind === "temperature" ? i.value : null)
		}), this.family === "xy" && (r = {
			kind: "xy",
			x: t("x", 0, 65534 / 65536, (e) => Math.round(e * 65536), i.kind === "xy" ? i.x : null),
			y: t("y", 0, 65534 / 65536, (e) => Math.round(e * 65536), i.kind === "xy" ? i.y : null)
		}), this.family === "rgbwaf" && (r = {
			kind: "rgbwaf",
			channels: It.map((e, n) => t(e, 0, 100, (e) => Math.round(e * 254 / 100), i.kind === "rgbwaf" ? i.channels[n] : null)),
			control: t("control", 128, 191, (e) => {
				if (!Number.isInteger(e)) throw Error("Control must be a whole native byte.");
				return e;
			}, i.kind === "rgbwaf" ? i.control : null)
		});
		let a = {
			level: n,
			color: r
		};
		if (!Nt(a)) throw Error("Scene values are invalid.");
		return a;
	}
	commit(e) {
		return this.pending ? this.pending : this.dirty ? (this.error = "", this.pending = Promise.resolve().then(() => e(this.setting())).then(() => {
			this.dirty = !1;
		}).catch((e) => {
			throw this.error = e.message, e;
		}).finally(() => {
			this.pending = void 0;
		}), this.pending) : Promise.resolve();
	}
}, It = [
	"red",
	"green",
	"blue",
	"white",
	"amber",
	"free"
];
function Lt(e) {
	if (!e || e.quality !== "complete" || e.stale) return null;
	try {
		let t = (e, t) => {
			if (e?.kind === "mask" && e.raw === t) return null;
			if (e?.kind === "value" && Mt(e.raw, 0, t - 1)) return e.raw;
			throw Error("Unresolved");
		}, n = t(e.level, 255), r = n === null ? { kind: "mask" } : {
			kind: "value",
			value: n
		}, i = { kind: "mask" }, a = 0, o = e.values;
		if (!o || typeof o != "object") return null;
		if (e.dt8 === !1) {
			if (e.color_type !== null) return null;
		} else if (e.dt8 === !0) {
			let n = t(e.color_type, 255);
			if (n === 32) i = {
				kind: "temperature",
				value: t(o.temperature, 65535)
			}, a = 1;
			else if (n === 16) i = {
				kind: "xy",
				x: t(o.x, 65535),
				y: t(o.y, 65535)
			}, a = 2;
			else if (n === 128) {
				if (i = {
					kind: "rgbwaf",
					channels: It.map((e) => t(o[e], 255)),
					control: t(o.control, 255)
				}, a = 7, i.control !== null && i.control < 128) return null;
			} else if (n !== null) return null;
		} else return null;
		let s = {
			level: r,
			color: i
		};
		return Object.keys(o).length === a && Nt(s) ? s : null;
	} catch {
		return null;
	}
}
// src/scene-controller.ts
function Rt(e, t, n) {
	if (!e || e.target !== n.item.target || e.target_generation !== n.generation || e.revision !== t.inventory.revision || e.bus !== n.bus.bus || e.slot !== n.item.slot || !Array.isArray(e.fixtures)) throw Error("Saved scene view changed. Refresh it.");
	let r = G(t).filter((e) => e.kind === "output" && e.bus.bus === n.bus.bus), i = /* @__PURE__ */ new Map();
	for (let t of e.fixtures) {
		let e = r.find((e) => e.item.target === t.fixture);
		if (!e || e.generation !== t.fixture_generation || i.has(e.key) || t.saved !== null && !Nt(t.saved)) throw Error("Saved scene view is incomplete or changed.");
		i.set(e.key, structuredClone(t.saved));
	}
	if (i.size !== r.length) throw Error("Saved scene view is incomplete.");
	return i;
}
var zt = class {
	constructor(e, t, n) {
		this.cancelled = !1, this.ui = e, this.initial = structuredClone(e.snapshot), this.epoch = e.client.views.epoch, this.bus = t, this.slot = n, this.revision = this.initial.inventory.revision;
		let r = G(this.initial);
		this.scene = r.find((e) => e.kind === "scene" && e.bus.bus === t && e.item.slot === n), this.outputs = r.filter((e) => e.kind === "output" && e.bus.bus === t);
	}
	assert(e = !0) {
		let t = this.ui.snapshot, n = this.initial.inventory, r = t.inventory, i = r.buses.find((e) => e.bus === this.bus), a = n.buses.find((e) => e.bus === this.bus);
		if (this.cancelled || e && this.ui.cancelled || this.ui.signingOut || this.epoch !== this.ui.client.views.epoch || !this.ui.client.views.ready) throw Error("Operation stopped. Review before continuing.");
		if (n.inventory_epoch !== r.inventory_epoch || r.revision !== this.revision || !i || !a || i.name !== a.name || t.phase !== "ready" || e && !K(t, i)) throw Error("Configuration or connection changed. Review the scene again.");
		if (this.scene) {
			let e = G(t).find((e) => e.key === this.scene.key);
			if (!e || e.generation !== this.scene.generation || e.item.target !== this.scene.item.target) throw Error("Selected scene changed. Review it again.");
		}
	}
	fixture(e) {
		this.assert();
		let t = this.ui.currentRows.find((t) => t.key === e.key);
		if (!t || t.generation !== e.generation || t.item.target !== e.item.target) throw Error("Selected fixture changed. Review it again.");
	}
	async saved() {
		if (this.assert(!1), !this.scene) return new Map(this.outputs.map((e) => [e.key, null]));
		let e = await this.ui.client.job(this.scene.generation, this.scene.item.target, "read_scene_settings");
		return this.assert(!1), Rt(e, this.initial, this.scene);
	}
	async inspect(e, t = this.slot, n = !1) {
		this.fixture(e);
		let r = await this.ui.client.job(e.generation, e.item.target, n ? "capture_scene_row" : "read_scene", { slot: t });
		this.fixture(e);
		let i = n ? r.fixture : r.target, a = n ? r.fixture_generation : r.target_generation;
		if (i !== e.item.target || a !== e.generation || r.revision !== this.revision || r.bus !== this.bus || r.slot !== t) throw Error("Fixture evidence changed. Inspect again.");
		return structuredClone(r);
	}
	async mutate(e, t, n) {
		this.fixture(e);
		let r = this.scene;
		if (!r) throw Error("Name this scene before saving settings.");
		let i = await this.ui.client.job(r.generation, r.item.target, t, {
			...structuredClone(n),
			fixture: e.item.target,
			expected_revision: this.revision
		});
		if (!Number.isSafeInteger(i.revision) || i.revision < this.revision || i.target !== r.item.target || i.target_generation !== r.generation || i.fixture !== e.item.target || i.fixture_generation !== e.generation || i.bus !== this.bus || i.slot !== this.slot) throw Error("Scene result changed. Inspect configuration before retrying.");
		if (this.cancelled || this.ui.cancelled || this.ui.signingOut || this.epoch !== this.ui.client.views.epoch || !this.ui.client.views.ready) throw Error("Operation stopped. Inspect saved settings before retrying.");
		if (await this.ui.changed(i.revision), this.revision = i.revision, this.fixture(e), i.outcome === "adopted" && i.comparison !== "matching" || i.outcome !== "verified" && i.outcome !== "adopted") throw Error("Saved settings are not confirmed in hardware. Inspect and repair explicitly.");
		return i;
	}
	async recall(e) {
		this.assert();
		let t = this.scene;
		if (!t) throw Error("Name this scene before recall.");
		let n = await this.ui.client.job(t.generation, t.item.target, "recall_scene", {
			expected_revision: this.revision,
			...e === void 0 ? {} : { group: e }
		});
		if (this.assert(), n.outcome !== "sent") throw Error("Recall was not confirmed as sent. Inspect the lights.");
		return n;
	}
};
function Bt(e) {
	return structuredClone(e);
}
async function Vt(e, t, n) {
	for (let r of t) e.fixture(r.row), n(`Saving scene ${e.slot} · ${r.row.item.name}`), await e.mutate(r.row, r.command, r.command === "set_scene_row" ? { setting: r.after } : { reviewed: r.reviewed });
}
// src/scenes.tsx
function Ht({ row: e, visible: t, view: n, makeScope: r }) {
	let i = Y(), [a, o] = H(null), [s, c] = H(null), [l, u] = H(-1), [d, f] = H(!1), p = n && n.revision === i.snapshot.inventory.revision && n.epoch === i.client.views.epoch && n.rows.has(e.key), m = p && n ? n.rows.get(e.key) : a?.original ?? null, h = a?.error.startsWith("Saved settings are not confirmed");
	U(() => () => {
		s && (s.cancelled = !0);
	}, [s]), U(() => {
		!p || !n || a && (a.dirty || a.pending) || l === n.revision && a || (o(new Ft(n.rows.get(e.key))), c(null), u(n.revision));
	}, [
		n,
		a?.dirty,
		a?.pending
	]);
	let g = K(i.snapshot, e.bus) && i.client.views.ready;
	function _() {
		a?.dirty || c(r());
	}
	let v = !!a?.dirty && (!s || s.revision !== i.snapshot.inventory.revision || s.epoch !== i.client.views.epoch), b = i.snapshot.identification, x = b?.target === e.item.target && [
		"starting",
		"blinking",
		"restoring"
	].includes(b.phase), S = Ae(i.snapshot, e).ColorCapabilities;
	function C() {
		a && s && a.dirty && (a.commit(async (t) => {
			if (i.busy) throw Error("Wait for the current operation, then save this draft.");
			i.busy++, i.error = "", i.touch();
			try {
				i.cancelled = !1, await s.mutate(e, "set_scene_row", { setting: t }), i.notice = `${e.item.name}: saved settings verified.`;
			} finally {
				i.busy--, i.touch();
			}
		}).catch(() => {}).finally(() => i.touch()), i.touch());
	}
	let w = !g || !p || !n?.key || !!i.busy || !!a?.pending || v, T = (t, n) => /* @__PURE__ */ J("label", {
		class: "scene-field",
		children: [n, /* @__PURE__ */ J("input", {
			type: "number",
			step: "any",
			"aria-label": `${n} ${e.item.name}`,
			value: a?.text(t) ?? "",
			disabled: w,
			placeholder: "MASK",
			onInput: (e) => {
				_(), a.change(t, e.currentTarget.value), i.touch();
			}
		})]
	}, t);
	return /* @__PURE__ */ J("article", {
		hidden: !t && !a?.dirty && !a?.error,
		class: `scene-fixture ${x ? "identifying" : ""}`,
		"data-scene-fixture": e.item.name,
		onBlur: (e) => {
			e.currentTarget.contains(e.relatedTarget) || C();
		},
		onKeyDown: (e) => {
			e.key === "Enter" && e.target.tagName !== "BUTTON" && (e.preventDefault(), C());
		},
		children: [/* @__PURE__ */ J("div", {
			class: "scene-fixture-title",
			children: [
				/* @__PURE__ */ J("span", {
					class: "address",
					children: e.item.short_address
				}),
				/* @__PURE__ */ J("strong", { children: e.item.name }),
				/* @__PURE__ */ J("span", {
					class: `pill ${p && m && !h ? "" : "unknown"}`,
					children: h ? "Saved · unconfirmed" : p ? m ? "Saved" : "Unmanaged" : "Refreshing"
				}),
				/* @__PURE__ */ J("button", {
					disabled: !g || !!i.busy || b?.phase === "restoring",
					"aria-label": `${x ? "Stop blink" : "Blink"} ${e.item.name}`,
					onClick: () => void i.run(() => i.blink(e)),
					children: x ? "Stop" : "Blink"
				})
			]
		}), a ? /* @__PURE__ */ J(y, { children: [
			/* @__PURE__ */ J("div", {
				class: "scene-editor",
				children: [
					/* @__PURE__ */ J("label", {
						class: "scene-field",
						children: ["Action", /* @__PURE__ */ J("select", {
							"aria-label": `Scene action ${e.item.name}`,
							value: a.action,
							disabled: w,
							onChange: (e) => {
								_(), a.chooseAction(e.currentTarget.value), i.touch();
							},
							children: [
								/* @__PURE__ */ J("option", {
									value: "ignore",
									children: "Leave unchanged"
								}),
								/* @__PURE__ */ J("option", {
									value: "off",
									children: "Off"
								}),
								/* @__PURE__ */ J("option", {
									value: "brightness",
									children: "Brightness"
								}),
								/* @__PURE__ */ J("option", {
									value: "keep",
									children: "Keep brightness"
								})
							]
						})]
					}),
					a.action === "brightness" && T("brightness", "Brightness (%)"),
					a.action !== "ignore" && /* @__PURE__ */ J("label", {
						class: "scene-field",
						children: ["Colour", /* @__PURE__ */ J("select", {
							"aria-label": `Scene colour ${e.item.name}`,
							value: a.family,
							disabled: w,
							onChange: (e) => {
								_(), a.chooseFamily(e.currentTarget.value), i.touch();
							},
							children: [/* @__PURE__ */ J("option", {
								value: "mask",
								children: "Keep colour"
							}), [
								["temperature", "White temperature"],
								["xy", "CIE xy"],
								["rgbwaf", "RGBWAF"]
							].map(([e, t]) => /* @__PURE__ */ J("option", {
								value: e,
								disabled: !!S?.supported_representations && !S.supported_representations.includes(e) && a.family !== e,
								children: t
							}))]
						})]
					}),
					a.action !== "ignore" && a.family === "temperature" && T("temperature", "Temperature (K)"),
					a.action !== "ignore" && a.family === "xy" && /* @__PURE__ */ J(y, { children: [T("x", "x"), T("y", "y")] }),
					a.action !== "ignore" && a.family === "rgbwaf" && /* @__PURE__ */ J(y, { children: [It.map((e) => T(e, `${e[0].toUpperCase() + e.slice(1)} (%)`)), /* @__PURE__ */ J("label", {
						class: "scene-field",
						children: ["RGBWAF control", /* @__PURE__ */ J("select", {
							"aria-label": `RGBWAF control ${e.item.name}`,
							value: a.text("control"),
							disabled: w,
							onChange: (e) => {
								_(), a.change("control", e.currentTarget.value), i.touch();
							},
							children: [
								/* @__PURE__ */ J("option", {
									value: "",
									children: "Keep control (MASK)"
								}),
								Array.from({ length: 64 }, (e, t) => /* @__PURE__ */ J("option", {
									value: 128 + t,
									children: ["Normalized · ", t.toString(2).padStart(6, "0")]
								})),
								a.text("control") !== "" && Number(a.text("control")) < 128 && /* @__PURE__ */ J("option", {
									value: a.text("control"),
									disabled: !0,
									children: ["Unsupported native mode ", a.text("control")]
								})
							]
						})]
					})] }),
					a.dirty && /* @__PURE__ */ J("button", {
						class: "primary scene-save",
						"aria-label": `Save scene settings ${e.item.name}`,
						disabled: w,
						onPointerDown: (e) => e.preventDefault(),
						onClick: C,
						children: a.pending ? "Saving…" : "✓"
					})
				]
			}),
			(v || a.error) && /* @__PURE__ */ J("p", {
				class: "error",
				role: "alert",
				children: [
					a.error || "Configuration changed while this row was being edited. Review it again.",
					" ",
					/* @__PURE__ */ J("button", {
						class: "text-button",
						disabled: !!a.pending,
						onClick: () => {
							o(null), u(-1);
						},
						children: "Discard draft"
					})
				]
			}),
			/* @__PURE__ */ J("div", {
				class: "scene-row-foot",
				children: [/* @__PURE__ */ J("span", {
					class: "hint",
					children: [Pt(m), a.dirty ? " · Unsaved draft" : ""]
				}), /* @__PURE__ */ J("button", {
					class: "text-button",
					onClick: () => f(!d),
					children: d ? "Hide native values" : "Native values"
				})]
			}),
			d && /* @__PURE__ */ J("div", {
				class: "scene-native",
				children: [
					/* @__PURE__ */ J("p", {
						class: "hint",
						children: [
							"Saved native codes; blank attributes are MASK. Colour support: ",
							S?.supported_representations?.join(", ") || "unconfirmed",
							S?.rgbwaf_assignments ? ` · Assigned: ${S.rgbwaf_assignments.join(", ")}` : "",
							"."
						]
					}),
					/* @__PURE__ */ J("pre", { children: JSON.stringify(m, null, 2) }),
					/* @__PURE__ */ J("button", {
						disabled: !g || !!i.busy,
						onClick: () => void i.run(() => i.read(e)),
						children: "Read capabilities"
					})
				]
			})
		] }) : /* @__PURE__ */ J("p", {
			class: "hint",
			children: "Loading saved settings…"
		})]
	});
}
function Ut({ bus: e }) {
	let t = Y(), [n, r] = H(0), [i, a] = H(""), [o, s] = H(""), [c, l] = H(null), [u, d] = H(""), [f, p] = H(0), [m, h] = H(null), [g, _] = H(null), [v, b] = H(1), [x, S] = H("bus"), [C, w] = H(null), [T, E] = H(""), [D, O] = H(null), k = _e(/* @__PURE__ */ new Set()), A = t.snapshot.inventory.revision, j = t.client.views.epoch, M = t.currentRows.find((t) => t.kind === "scene" && t.bus.bus === e.bus && t.item.slot === n), ee = t.currentRows.filter((t) => t.kind === "output" && t.bus.bus === e.bus), N = ee.filter((e) => ke(i, e.item.name)), te = K(t.snapshot, e) && t.client.views.ready, P = !!c && c.revision === A && c.epoch === j && c.key === M?.key;
	function F() {
		for (let e of k.current) e.cancelled && k.current.delete(e);
		let r = new zt(t, e.bus, n);
		return k.current.add(r), r;
	}
	U(() => () => {
		for (let e of k.current) e.cancelled = !0;
		k.current.clear();
	}, [n]), U(() => {
		let e = F(), n = !0;
		return d(""), l(null), t.client.views.ready && e.saved().then((t) => {
			n && l({
				revision: e.revision,
				epoch: e.epoch,
				key: e.scene?.key,
				rows: t
			});
		}).catch((e) => {
			n && d(e.message);
		}), () => {
			n = !1, e.cancelled = !0, k.current.delete(e);
		};
	}, [
		n,
		A,
		j,
		t.client.views.ready,
		f
	]);
	function ne(e) {
		if (t.busy) {
			t.cancelled = !0;
			for (let e of k.current) e.cancelled = !0;
		}
		r(e), s(""), h(null), _(null);
	}
	function I() {
		for (let e of g ?? []) e.scope.cancelled = !0;
		_(null);
	}
	function L(e) {
		if (!P || !c?.rows.has(e.key)) throw Error("Wait for complete saved settings.");
		return c.rows.get(e.key);
	}
	function R(e, t, n, r = []) {
		t.assert(), h({
			title: e,
			scope: t,
			changes: Bt(n),
			unresolved: [...r]
		});
	}
	function z() {
		t.cancelled = !1, R("Clear selected fixtures", F(), N.map((e) => ({
			row: e,
			before: L(e),
			after: jt(),
			command: "set_scene_row"
		})));
	}
	async function B() {
		t.cancelled = !1;
		let e = F(), r = [], i = [], a = !1;
		try {
			for (let a of N) {
				e.fixture(a), t.progress = `Capturing current output · ${a.item.name}`, t.touch();
				let o = await e.inspect(a, n, !0);
				Nt(o.candidate) ? r.push({
					row: a,
					before: L(a),
					after: o.candidate,
					command: "set_scene_row"
				}) : i.push(a.item.name);
			}
			R("Review captured output", e, r, i), a = !0;
		} finally {
			a || (e.cancelled = !0), t.progress = "", t.touch();
		}
	}
	async function re() {
		t.cancelled = !1;
		let r = F(), i = new zt(t, e.bus, v);
		k.current.add(i);
		let a = !1;
		try {
			if (!i.scene) throw Error("Choose a named source scene.");
			let e = await i.saved();
			r.assert(), i.assert(), R(`Copy saved scene ${v} → ${n}`, r, N.filter((t) => e.get(t.key) !== null).map((t) => ({
				row: t,
				before: L(t),
				after: e.get(t.key),
				command: "set_scene_row"
			}))), a = !0;
		} finally {
			i.cancelled = !0, a || (r.cancelled = !0);
		}
	}
	async function ie() {
		let e = m;
		if (e) {
			h(null), t.cancelled = !1;
			try {
				await Vt(e.scope, e.changes, (e) => {
					t.progress = e, t.touch();
				}), t.notice = "Reviewed scene settings saved and verified.";
			} finally {
				e.scope.cancelled = !0, t.progress = "", p((e) => e + 1), t.touch();
			}
		}
	}
	async function ae(e) {
		t.cancelled = !1;
		let r = F(), i = [];
		try {
			for (let a of e ? Array.from({ length: 16 }, (e, t) => t) : [n]) for (let e of r.outputs) {
				r.fixture(e), t.progress = `Verifying scene ${a} · ${e.item.name}`, t.touch();
				let n = await r.inspect(e, a);
				i.push({
					scope: r,
					row: e,
					slot: a,
					report: n,
					candidate: Lt(n.stored)
				});
			}
			t.notice = "Native scene reads complete. Review differences and unresolved fixtures.";
		} finally {
			i.length && !r.cancelled && r.epoch === t.client.views.epoch && r.revision === t.snapshot.inventory.revision ? _(i) : r.cancelled = !0, t.progress = "", t.touch();
		}
	}
	function oe(e) {
		if (!g) return;
		t.cancelled = !1;
		let r = g.filter((e) => e.slot === n && ke(i, e.row.item.name)), a = F();
		for (let e of r) if (e.scope.assert(), e.scope.revision !== a.revision) throw Error("Verification is stale. Verify again.");
		let o = r.filter((t) => t.candidate && (e === "adopt_scene_row" || t.report.saved !== null) && t.report.comparison !== "matching").map((t) => ({
			row: t.row,
			before: L(t.row),
			after: e === "adopt_scene_row" ? t.candidate : t.report.saved,
			reviewed: t.report.stored,
			command: e
		}));
		I(), R(e === "adopt_scene_row" ? "Adopt reviewed native settings" : "Repair saved settings", a, o);
	}
	async function se() {
		let r = F();
		try {
			r.assert(!1);
			let i = await t.client.job(W(r.initial.inventory.inventory_epoch, r.revision), null, "create_scene", {
				bus: e.bus,
				slot: n,
				name: o || `Scene${n + 1}`,
				expected_revision: r.revision
			});
			if (await t.changed(i.revision), r.cancelled || r.epoch !== t.client.views.epoch) return;
			t.notice = "Scene name saved.", s("");
		} finally {
			r.cancelled = !0;
		}
	}
	async function V() {
		t.cancelled = !1;
		let e = F();
		try {
			await e.recall(x === "bus" ? void 0 : Number(x)), t.notice = "Scene recall sent. Read fixtures for current output.";
		} finally {
			e.cancelled = !0;
		}
	}
	let ce = !te || !P || !M || !!t.busy || !N.length;
	return /* @__PURE__ */ J(y, { children: [
		/* @__PURE__ */ J(Ve, {}),
		/* @__PURE__ */ J("div", {
			class: "scene-mobile-slot",
			children: /* @__PURE__ */ J("label", { children: ["Scene", /* @__PURE__ */ J("select", {
				"aria-label": "Scene slot",
				value: n,
				onChange: (e) => ne(Number(e.currentTarget.value)),
				children: Array.from({ length: 16 }, (t, n) => /* @__PURE__ */ J("option", {
					value: n,
					children: [
						n,
						" · ",
						e.scenes?.find((e) => e.slot === n)?.name ?? "Unnamed"
					]
				}))
			})] })
		}),
		/* @__PURE__ */ J("div", {
			class: "groups-layout scene-layout",
			children: [/* @__PURE__ */ J("aside", {
				class: "slots scene-slots",
				children: [/* @__PURE__ */ J("h2", { children: "Scenes" }), Array.from({ length: 16 }, (t, r) => /* @__PURE__ */ J("button", {
					class: r === n ? "selected" : "",
					onClick: () => ne(r),
					children: [/* @__PURE__ */ J("span", { children: r }), e.scenes?.find((e) => e.slot === r)?.name ?? "Unnamed"]
				}))]
			}), /* @__PURE__ */ J("section", { children: [
				/* @__PURE__ */ J("div", {
					class: "section-heading",
					children: [/* @__PURE__ */ J("h2", { children: ["Scene ", n] }), /* @__PURE__ */ J("button", {
						disabled: !te || !!t.busy,
						onClick: () => void t.run(() => ae(!0)),
						children: "Verify all 16"
					})]
				}),
				M ? /* @__PURE__ */ J("div", {
					class: "card group-title",
					children: [
						/* @__PURE__ */ J(Ue, { row: M }),
						/* @__PURE__ */ J("span", {
							class: "hint",
							children: ["Slot ", n]
						}),
						/* @__PURE__ */ J("button", {
							"aria-label": "Scene actions",
							onClick: () => {
								E(M.item.description ?? ""), w(structuredClone(M));
							},
							children: "⋮"
						})
					]
				}) : /* @__PURE__ */ J("div", {
					class: "card",
					children: [
						/* @__PURE__ */ J("h3", { children: ["Unnamed scene ", n] }),
						/* @__PURE__ */ J("p", {
							class: "muted",
							children: "Name this slot before saving settings. Browsing leaves the fixtures unchanged."
						}),
						/* @__PURE__ */ J("div", {
							class: "toolbar",
							children: [/* @__PURE__ */ J("input", {
								"aria-label": "New scene name",
								placeholder: `Scene${n + 1}`,
								value: o,
								onInput: (e) => s(e.currentTarget.value)
							}), /* @__PURE__ */ J("button", {
								class: "primary",
								disabled: !!t.busy || !t.client.views.ready,
								onClick: () => void t.run(se),
								children: "Create scene"
							})]
						})
					]
				}),
				/* @__PURE__ */ J("div", {
					class: "toolbar scene-actions",
					children: [
						/* @__PURE__ */ J("button", {
							class: "primary",
							disabled: !te || !M || !!t.busy,
							onClick: () => void t.run(V),
							children: "Recall"
						}),
						/* @__PURE__ */ J("select", {
							"aria-label": "Recall scope",
							value: x,
							disabled: !!t.busy,
							onChange: (e) => S(e.currentTarget.value),
							children: [/* @__PURE__ */ J("option", {
								value: "bus",
								children: "Entire bus"
							}), Array.from({ length: 16 }, (t, n) => /* @__PURE__ */ J("option", {
								value: n,
								children: [
									"Group ",
									n,
									e.groups?.find((e) => e.group_address === n)?.name ? ` · ${e.groups.find((e) => e.group_address === n).name}` : ""
								]
							}))]
						}),
						/* @__PURE__ */ J("button", {
							disabled: !te || !!t.busy,
							onClick: () => void t.run(() => ae(!1)),
							children: "Verify scene"
						})
					]
				}),
				/* @__PURE__ */ J("div", {
					class: "toolbar filter scene-filter",
					children: [/* @__PURE__ */ J("input", {
						type: "search",
						"aria-label": "Scene fixture pattern",
						placeholder: "Filter fixtures · e.g. Light*",
						value: i,
						onInput: (e) => a(e.currentTarget.value)
					}), /* @__PURE__ */ J("span", {
						class: "hint",
						children: [
							N.length,
							" / ",
							ee.length,
							" fixtures"
						]
					})]
				}),
				/* @__PURE__ */ J("div", {
					class: "toolbar scene-tools",
					children: [
						/* @__PURE__ */ J("button", {
							disabled: ce,
							onClick: () => void t.run(B),
							children: "Capture current"
						}),
						/* @__PURE__ */ J("button", {
							disabled: ce,
							onClick: () => void t.run(async () => z()),
							children: "Clear selected"
						}),
						/* @__PURE__ */ J("select", {
							"aria-label": "Copy source scene",
							value: v,
							onChange: (e) => b(Number(e.currentTarget.value)),
							children: Array.from({ length: 16 }, (t, n) => /* @__PURE__ */ J("option", {
								value: n,
								children: [
									"From ",
									n,
									" · ",
									e.scenes?.find((e) => e.slot === n)?.name ?? "Unnamed"
								]
							}))
						}),
						/* @__PURE__ */ J("button", {
							disabled: ce || v === n,
							onClick: () => void t.run(re),
							children: "Copy saved"
						})
					]
				}),
				/* @__PURE__ */ J("p", {
					class: "hint scene-explanation",
					children: "Settings change native scene memory. Recall changes the lights. Blank colour attributes keep that value (MASK)."
				}),
				u && /* @__PURE__ */ J("p", {
					class: "error",
					role: "alert",
					children: [
						u,
						" ",
						/* @__PURE__ */ J("button", {
							onClick: () => p((e) => e + 1),
							children: "Reload saved settings"
						})
					]
				}),
				/* @__PURE__ */ J("div", {
					class: "scene-fixtures",
					children: ee.map((e) => /* @__PURE__ */ J(Ht, {
						row: e,
						visible: ke(i, e.item.name),
						view: c,
						makeScope: F
					}, `${n}:${e.key}`))
				}),
				!N.length && /* @__PURE__ */ J("p", {
					class: "empty",
					children: "No matching fixtures."
				}),
				m && /* @__PURE__ */ J(Z, {
					title: m.title,
					close: () => {
						m.scope.cancelled = !0, h(null);
					},
					children: [
						/* @__PURE__ */ J("p", { children: [
							"Scene ",
							m.scope.slot,
							" · ",
							m.changes.length,
							" fixtures. Current output and unselected fixtures are preserved."
						] }),
						m.changes.map((e) => /* @__PURE__ */ J("section", {
							class: "verify-row",
							children: [
								/* @__PURE__ */ J("h3", { children: e.row.item.name }),
								/* @__PURE__ */ J("p", { children: [
									Pt(e.before),
									" → ",
									Pt(e.after)
								] }),
								/* @__PURE__ */ J("details", { children: [/* @__PURE__ */ J("summary", { children: "Native before / after" }), /* @__PURE__ */ J("pre", { children: JSON.stringify({
									before: e.before,
									after: e.after
								}, null, 2) })] })
							]
						}, e.row.key)),
						!!m.unresolved.length && /* @__PURE__ */ J("p", {
							class: "error",
							children: [
								"Unresolved; excluded: ",
								m.unresolved.join(", "),
								"."
							]
						}),
						/* @__PURE__ */ J("div", {
							class: "dialog-actions",
							children: [/* @__PURE__ */ J("button", {
								onClick: () => {
									m.scope.cancelled = !0, h(null);
								},
								children: "Cancel"
							}), /* @__PURE__ */ J("button", {
								class: "primary",
								disabled: !!t.busy || !m.changes.length,
								onClick: () => void t.run(ie),
								children: "Apply reviewed settings"
							})]
						})
					]
				}),
				g && /* @__PURE__ */ J(Z, {
					title: "Verify native scenes",
					close: I,
					children: [
						/* @__PURE__ */ J("p", {
							class: "hint",
							children: [
								"Results cover completed native reads. Adoption and repair below use this slot (",
								n,
								") and the current name filter."
							]
						}),
						g.map((e) => /* @__PURE__ */ J("section", {
							class: "verify-row",
							children: [
								/* @__PURE__ */ J("h3", { children: [
									e.row.item.name,
									" · scene ",
									e.slot
								] }),
								/* @__PURE__ */ J("p", { children: [e.candidate ? {
									matching: "Matching",
									different: "Different",
									unmanaged: "Unmanaged",
									unresolved: "Unresolved"
								}[e.report.comparison] ?? "Unresolved" : "Unresolved", e.candidate ? "" : " · native data cannot be adopted"] }),
								/* @__PURE__ */ J("p", {
									class: "hint",
									children: ["Saved: ", Pt(e.report.saved)]
								}),
								/* @__PURE__ */ J("details", { children: [/* @__PURE__ */ J("summary", { children: "Native evidence" }), /* @__PURE__ */ J("pre", { children: JSON.stringify(e.report.stored, null, 2) })] })
							]
						}, `${e.slot}:${e.row.key}`)),
						/* @__PURE__ */ J("div", {
							class: "dialog-actions",
							children: [/* @__PURE__ */ J("button", {
								disabled: !M || !!t.busy || !P,
								onClick: () => void t.run(async () => oe("adopt_scene_row")),
								children: "Review adoption"
							}), /* @__PURE__ */ J("button", {
								disabled: !M || !!t.busy || !P,
								onClick: () => void t.run(async () => oe("repair_scene_row")),
								children: "Review repair"
							})]
						})
					]
				}),
				C && /* @__PURE__ */ J(Z, {
					title: `Scene · ${C.item.name}`,
					close: () => w(null),
					children: [/* @__PURE__ */ J("label", {
						class: "field",
						children: ["Description", /* @__PURE__ */ J("textarea", {
							"aria-label": "Scene description",
							value: T,
							onInput: (e) => E(e.currentTarget.value)
						})]
					}), /* @__PURE__ */ J("div", {
						class: "dialog-actions",
						children: [/* @__PURE__ */ J("button", {
							disabled: !!t.busy,
							onClick: () => void t.run(async () => {
								await t.metadata("describe", C.item.target, C.generation, { description: T || null }), w(null);
							}),
							children: "Save description"
						}), /* @__PURE__ */ J("button", {
							disabled: !!t.busy,
							onClick: () => {
								w(null), O(C);
							},
							children: "Remove scene name"
						})]
					})]
				}),
				D && /* @__PURE__ */ J(Z, {
					title: "Remove scene name",
					close: () => O(null),
					children: [/* @__PURE__ */ J("p", { children: [
						"Remove ",
						D.item.name,
						" and its saved expectations from the bridge. Native fixture contents remain in slot ",
						D.item.slot,
						"."
					] }), /* @__PURE__ */ J("div", {
						class: "dialog-actions",
						children: [/* @__PURE__ */ J("button", {
							onClick: () => O(null),
							children: "Cancel"
						}), /* @__PURE__ */ J("button", {
							disabled: !!t.busy,
							onClick: () => void t.run(async () => {
								await t.remove(D), O(null);
							}),
							children: "Remove scene"
						})]
					})]
				})
			] })]
		})
	] });
}
// src/reassignment.ts
var Wt = class {
	constructor(e, t, n) {
		this.cancelled = !1, this.ui = e, this.initial = structuredClone(e.snapshot), this.epoch = e.client.views.epoch, this.bus = t, this.revision = this.initial.inventory.revision, this.spaces = n === "both" ? ["outputs", "inputs"] : [n];
	}
	assert() {
		let e = this.ui, t = e.snapshot, n = t.inventory.buses.find((e) => e.bus === this.bus);
		if (this.cancelled || e.cancelled || e.signingOut || !e.client.views.ready || e.client.views.epoch !== this.epoch) throw Error("Reassignment stopped. Completed changes remain saved.");
		if (!q(this.initial, t, this.revision, this.bus) || t.phase !== "ready" || !n || !K(t, n)) throw Error("Configuration or bus connection changed. Completed changes remain saved; review before continuing.");
	}
	async perform(e, t, n) {
		this.assert();
		let r = await this.ui.client.job(W(this.initial.inventory.inventory_epoch, this.revision), null, e, {
			bus: this.bus,
			...e.startsWith("assign_next_") ? {} : { space: t },
			...n === void 0 ? {} : { address: n },
			expected_revision: this.revision
		}), i = r.probe ?? r;
		if (!Number.isSafeInteger(r.revision) || r.revision < this.revision || i.bus !== this.bus || i.space !== t) throw Error("Reassignment result changed. Inspect configuration before retrying.");
		if (this.cancelled || this.ui.cancelled || this.ui.signingOut || this.epoch !== this.ui.client.views.epoch || !this.ui.client.views.ready) throw Error("Reassignment stopped. Inspect saved changes before retrying.");
		if (await this.ui.changed(r.revision), this.revision = r.revision, this.assert(), r.probe?.types_complete && r.target) {
			let e = this.ui.currentRows.find((e) => e.item.target === r.target);
			e && this.ui.hints.set(e.key, r.probe.device_types);
		}
		return r;
	}
	async run(e) {
		for (let t of this.spaces) {
			if (e(`Retiring saved ${t} and clearing their addresses`), (await this.perform("reset_address_space", t)).outcome !== "cleared") throw Error("Saved identities were retired, but address clearing is unconfirmed. Reassignment stopped; review before retrying.");
			let n = !0;
			for (let r = 0; r < 64; r++) {
				e(`Assigning ${t} · ${r} found`);
				let i = await this.perform(t === "outputs" ? "assign_next_output" : "assign_next_input", t);
				if (i.outcome === `no_unaddressed_${t === "outputs" ? "output" : "input"}`) {
					n = !1;
					break;
				}
				if (i.outcome !== "assigned" || !Number.isInteger(i.address) || i.address < 0 || i.address > 63) throw Error("Address assignment is unconfirmed or no free address remains. Completed changes remain saved.");
				await this.perform("discover_address", t, i.address);
			}
			n && (this.ui.notice = `All 64 ${t} addresses are assigned. Additional unaddressed devices may remain.`);
			for (let n = 0; n < 64; n++) e(`Refreshing ${t} · address ${n}`), await this.perform("discover_address", t, n);
			let r = G(this.ui.snapshot).filter((e) => e.bus.bus === this.bus && (t === "outputs" ? e.kind === "output" : e.kind === "instance"));
			for (let t of r) this.assert(), e(`Reading ${t.item.name}`), await this.ui.read(t), this.assert();
		}
		this.assert(), this.ui.notice = this.ui.notice || "Reassignment, discovery and current-value refresh finished. Identify the new default names on this screen.";
	}
};
// src/reassignment-ui.tsx
function Gt({ bus: e, settings: t }) {
	let n = Y(), [r, i] = H(!1), [a, o] = H(!1), [s, c] = H("outputs"), [l, u] = H(null), d = _e(null);
	U(() => () => {
		d.current && (d.current.cancelled = !0);
	}, []);
	let f = n.client.views.ready && K(n.snapshot, e), p = l ? G(l.initial).filter((e) => e.bus.bus === l.bus && (e.kind === "output" ? l.spaces.includes("outputs") : (e.kind === "input" || e.kind === "instance") && l.spaces.includes("inputs"))) : [];
	return /* @__PURE__ */ J(y, { children: [
		/* @__PURE__ */ J("button", {
			onClick: () => i(!0),
			children: "Bus actions"
		}),
		r && /* @__PURE__ */ J(Z, {
			title: `${n.snapshot.identity.controller}/${e.name}`,
			close: () => i(!1),
			children: /* @__PURE__ */ J("div", {
				class: "menu-actions",
				children: [/* @__PURE__ */ J("button", {
					onClick: () => {
						i(!1), t();
					},
					children: "Bus settings"
				}), /* @__PURE__ */ J("button", {
					class: "danger",
					disabled: !f || !!n.busy,
					onClick: () => {
						i(!1), o(!0);
					},
					children: "Reassign addresses"
				})]
			})
		}),
		a && /* @__PURE__ */ J(Z, {
			title: "Reassign addresses",
			close: () => o(!1),
			children: [
				/* @__PURE__ */ J("label", {
					class: "field",
					children: ["Address space", /* @__PURE__ */ J("select", {
						"aria-label": "Reassignment address space",
						value: s,
						onChange: (e) => c(e.currentTarget.value),
						children: [
							/* @__PURE__ */ J("option", {
								value: "outputs",
								children: "Outputs only"
							}),
							/* @__PURE__ */ J("option", {
								value: "inputs",
								children: "Inputs only"
							}),
							/* @__PURE__ */ J("option", {
								value: "both",
								children: "Outputs and inputs"
							})
						]
					})]
				}),
				/* @__PURE__ */ J("p", { children: "Clear the selected short addresses and discover those devices with fresh default names." }),
				/* @__PURE__ */ J("div", {
					class: "dialog-actions",
					children: [/* @__PURE__ */ J("button", {
						onClick: () => o(!1),
						children: "Cancel"
					}), /* @__PURE__ */ J("button", {
						class: "primary",
						disabled: !f || !!n.busy,
						onClick: () => {
							u(new Wt(n, e.bus, s)), o(!1);
						},
						children: "Review reassignment"
					})]
				})
			]
		}),
		l && /* @__PURE__ */ J(Z, {
			title: "Confirm address reassignment",
			close: () => u(null),
			children: [
				/* @__PURE__ */ J("p", { children: [
					/* @__PURE__ */ J("strong", { children: [
						l.initial.identity.controller,
						"/",
						l.initial.inventory.buses.find((e) => e.bus === l.bus).name
					] }),
					" · ",
					l.spaces.join(" and ")
				] }),
				/* @__PURE__ */ J("p", { children: [
					"The saved names below will be removed. ",
					l.spaces.includes("outputs") && "Saved output group memberships and scene expectations will be removed. ",
					"New devices receive default names such as ",
					l.spaces.includes("outputs") ? "Light1" : "Input1",
					"."
				] }),
				/* @__PURE__ */ J("p", { children: "Live light values, native group memberships and native scene memory stay unchanged. Use Verify groups/scenes afterward to review them." }),
				/* @__PURE__ */ J("details", { children: [/* @__PURE__ */ J("summary", { children: [
					p.filter((e) => e.kind !== "instance").length,
					" saved devices · ",
					p.filter((e) => e.kind === "instance").length,
					" input instances"
				] }), /* @__PURE__ */ J("ul", { children: p.map((e) => /* @__PURE__ */ J("li", { children: [
					e.item.name,
					" ",
					/* @__PURE__ */ J("span", {
						class: "hint",
						children: e.kind
					})
				] }, e.key)) })] }),
				/* @__PURE__ */ J("p", {
					class: "hint",
					children: "Selected spaces are processed one at a time. Stopping keeps completed changes. An unconfirmed step stops further work."
				}),
				/* @__PURE__ */ J("div", {
					class: "dialog-actions",
					children: [/* @__PURE__ */ J("button", {
						onClick: () => u(null),
						children: "Cancel"
					}), /* @__PURE__ */ J("button", {
						class: "danger",
						disabled: !!n.busy || !f,
						onClick: () => {
							if (!l) return;
							let e = l;
							d.current = e, u(null), n.cancelled = !1, n.run(async () => {
								try {
									await e.run((e) => {
										n.progress = e, n.touch();
									});
								} finally {
									n.progress = "", d.current === e && (d.current = null), n.touch();
								}
							});
						},
						children: "Clear and reassign"
					})]
				})
			]
		})
	] });
}
// src/app.tsx
function Kt({ number: e, bus: t, close: n }) {
	let r = Y(), [i, a] = H(t?.name ?? `Bus${e + 1}`), [o, s] = H(t?.enabled ?? !0);
	return /* @__PURE__ */ J(Z, {
		title: `Bus ${e + 1} settings`,
		close: n,
		children: /* @__PURE__ */ J("form", {
			onSubmit: (t) => {
				t.preventDefault(), r.run(async () => {
					await r.configureBus(e, i, o), n();
				});
			},
			children: [
				/* @__PURE__ */ J("label", {
					class: "field",
					children: ["Bus name", /* @__PURE__ */ J("input", {
						required: !0,
						"aria-label": "Bus name",
						value: i,
						onInput: (e) => a(e.currentTarget.value)
					})]
				}),
				/* @__PURE__ */ J("label", {
					class: "check-field",
					children: [/* @__PURE__ */ J("input", {
						type: "checkbox",
						checked: o,
						onChange: (e) => s(e.currentTarget.checked)
					}), "Enable bus"]
				}),
				/* @__PURE__ */ J("p", {
					class: "hint",
					children: "Saved devices keep their identity when the bus is renamed or disabled."
				}),
				/* @__PURE__ */ J("div", {
					class: "dialog-actions",
					children: [/* @__PURE__ */ J("button", {
						type: "button",
						onClick: n,
						children: "Cancel"
					}), /* @__PURE__ */ J("button", {
						class: "primary",
						disabled: !!r.busy || !r.client.views.ready,
						children: "Save bus"
					})]
				})
			]
		})
	});
}
function qt() {
	let e = Y(), [t, n] = H(null), r = e.snapshot;
	return /* @__PURE__ */ J(y, { children: [
		/* @__PURE__ */ J("header", {
			class: "page-heading",
			children: /* @__PURE__ */ J("div", { children: [
				/* @__PURE__ */ J("p", {
					class: "eyebrow",
					children: "CONTROLLER"
				}),
				/* @__PURE__ */ J("h1", { children: r.identity.controller }),
				/* @__PURE__ */ J("p", {
					class: "muted",
					children: "DALI bus management"
				})
			] })
		}),
		/* @__PURE__ */ J("div", {
			class: "summary-grid",
			children: [
				/* @__PURE__ */ J("div", {
					class: "card",
					children: [/* @__PURE__ */ J("span", {
						class: "muted",
						children: "Controller"
					}), /* @__PURE__ */ J("h2", { children: r.phase })]
				}),
				/* @__PURE__ */ J("div", {
					class: "card",
					children: [/* @__PURE__ */ J("span", {
						class: "muted",
						children: "UART / HAT"
					}), /* @__PURE__ */ J("h2", { children: r.hardware.state })]
				}),
				/* @__PURE__ */ J("div", {
					class: "card",
					children: [/* @__PURE__ */ J("span", {
						class: "muted",
						children: "Inventory revision"
					}), /* @__PURE__ */ J("h2", { children: r.inventory.revision })]
				})
			]
		}),
		/* @__PURE__ */ J("div", {
			class: "section-heading",
			children: [/* @__PURE__ */ J("h2", { children: "Buses" }), /* @__PURE__ */ J("span", {
				class: "hint",
				children: "Select a bus to discover and identify its devices."
			})]
		}),
		/* @__PURE__ */ J("div", {
			class: "bus-grid",
			children: Array.from({ length: 4 }, (e, t) => {
				let i = r.inventory.buses.find((e) => e.bus === t);
				return /* @__PURE__ */ J("article", {
					class: "card bus-card",
					children: [
						/* @__PURE__ */ J("div", {
							class: "section-heading",
							children: [/* @__PURE__ */ J("h2", { children: i?.name ?? `Bus${t + 1}` }), /* @__PURE__ */ J("span", {
								class: `pill ${i && K(r, i) ? "" : "unknown"}`,
								children: i ? i.enabled ? "Enabled" : "Disabled" : "Not configured"
							})]
						}),
						/* @__PURE__ */ J("p", {
							class: "muted",
							children: [
								i?.outputs.length ?? 0,
								" outputs · ",
								i?.inputs?.length ?? 0,
								" input modules · ",
								i?.groups?.length ?? 0,
								" groups"
							]
						}),
						/* @__PURE__ */ J("div", {
							class: "toolbar",
							children: [i && /* @__PURE__ */ J(X, {
								class: "button primary",
								href: `/buses/${t}`,
								children: "Open bus"
							}), /* @__PURE__ */ J("button", {
								onClick: () => n(t),
								children: i ? "Bus settings" : "Set up bus"
							})]
						})
					]
				}, t);
			})
		}),
		t !== null && /* @__PURE__ */ J(Kt, {
			number: t,
			bus: r.inventory.buses.find((e) => e.bus === t),
			close: () => n(null)
		})
	] });
}
function Jt({ number: e, tab: t }) {
	let n = Y(), r = n.snapshot.inventory.buses.find((t) => t.bus === e), [i, a] = H(!1);
	return /* @__PURE__ */ J(y, { children: [
		/* @__PURE__ */ J("header", {
			class: "page-heading",
			children: [/* @__PURE__ */ J("div", { children: [
				/* @__PURE__ */ J("p", {
					class: "eyebrow",
					children: "BUS MANAGEMENT"
				}),
				/* @__PURE__ */ J("h1", { children: [
					n.snapshot.identity.controller,
					"/",
					r?.name ?? `Bus${e + 1}`
				] }),
				/* @__PURE__ */ J("p", {
					class: "muted",
					children: "Discover, identify and name the devices on this bus."
				})
			] }), /* @__PURE__ */ J(y, { children: r ? /* @__PURE__ */ J(Gt, {
				bus: r,
				settings: () => a(!0)
			}, `${e}-${t}`) : /* @__PURE__ */ J("button", {
				onClick: () => a(!0),
				children: "Bus settings"
			}) })]
		}),
		r ? /* @__PURE__ */ J(y, { children: [/* @__PURE__ */ J("nav", {
			class: "tabs",
			"aria-label": "Bus pages",
			children: [
				["", "Devices"],
				["/groups", "Groups"],
				["/scenes", "Scenes"]
			].map(([n, r]) => /* @__PURE__ */ J(X, {
				class: t === n ? "selected" : "",
				href: `/buses/${e}${n}`,
				children: r
			}))
		}), t === "/groups" ? /* @__PURE__ */ J(At, { bus: r }, `groups-${e}`) : t === "/scenes" ? /* @__PURE__ */ J(Ut, { bus: r }, `scenes-${e}`) : /* @__PURE__ */ J(wt, { bus: r }, `devices-${e}`)] }) : /* @__PURE__ */ J("div", {
			class: "card",
			children: [
				/* @__PURE__ */ J("h2", { children: "Set up this bus" }),
				/* @__PURE__ */ J("p", {
					class: "muted",
					children: "Save a bus name and enable it before starting discovery."
				}),
				/* @__PURE__ */ J("button", {
					class: "primary",
					onClick: () => a(!0),
					children: "Set up bus"
				})
			]
		}),
		i && /* @__PURE__ */ J(Kt, {
			number: e,
			bus: r,
			close: () => a(!1)
		})
	] });
}
function Yt() {
	let e = Y().snapshot;
	function t() {
		let t = URL.createObjectURL(new Blob([JSON.stringify(e.inventory, null, 2)], { type: "application/json" })), n = document.createElement("a");
		n.href = t, n.download = "dali-inventory-report.json", n.click(), setTimeout(() => URL.revokeObjectURL(t), 0);
	}
	return /* @__PURE__ */ J(y, { children: [
		/* @__PURE__ */ J("header", {
			class: "page-heading",
			children: /* @__PURE__ */ J("div", { children: [/* @__PURE__ */ J("p", {
				class: "eyebrow",
				children: "CONTROLLER"
			}), /* @__PURE__ */ J("h1", { children: "Settings" })] })
		}),
		/* @__PURE__ */ J("section", {
			class: "card settings-section",
			children: [
				/* @__PURE__ */ J("h2", { children: e.identity.controller }),
				/* @__PURE__ */ J("dl", { children: [
					/* @__PURE__ */ J("dt", { children: "Version" }),
					/* @__PURE__ */ J("dd", { children: e.identity.version }),
					/* @__PURE__ */ J("dt", { children: "Build UTC" }),
					/* @__PURE__ */ J("dd", { children: e.identity.build_utc }),
					/* @__PURE__ */ J("dt", { children: "Inventory epoch" }),
					/* @__PURE__ */ J("dd", {
						class: "mono",
						children: e.inventory.inventory_epoch
					}),
					/* @__PURE__ */ J("dt", { children: "Revision" }),
					/* @__PURE__ */ J("dd", { children: e.inventory.revision })
				] }),
				/* @__PURE__ */ J("p", {
					class: "hint",
					children: "Listener, MQTT, UART and password options are supplied when the bridge starts. Bus settings are available on each bus page."
				})
			]
		}),
		/* @__PURE__ */ J("section", {
			class: "card settings-section",
			children: [
				/* @__PURE__ */ J("h2", { children: "Inventory report" }),
				/* @__PURE__ */ J("p", {
					class: "muted",
					children: "Download the current published inventory and identity history. This report is not a restorable configuration backup."
				}),
				/* @__PURE__ */ J("button", {
					onClick: t,
					children: "Download inventory report"
				})
			]
		})
	] });
}
function Xt() {
	let e = Y(), t = e.snapshot;
	return /* @__PURE__ */ J(y, { children: [
		/* @__PURE__ */ J("header", {
			class: "page-heading",
			children: /* @__PURE__ */ J("div", { children: [/* @__PURE__ */ J("p", {
				class: "eyebrow",
				children: "CONTROLLER"
			}), /* @__PURE__ */ J("h1", { children: "Diagnostics" })] })
		}),
		/* @__PURE__ */ J("div", {
			class: "summary-grid",
			children: [
				/* @__PURE__ */ J("section", {
					class: "card",
					children: [
						/* @__PURE__ */ J("h2", { children: "UART / HAT" }),
						/* @__PURE__ */ J("p", { children: t.hardware.state }),
						t.hardware.buses?.map((e) => /* @__PURE__ */ J("p", {
							class: "hint",
							children: [
								"Bus ",
								e.bus + 1,
								" · status ",
								e.status
							]
						}))
					]
				}),
				/* @__PURE__ */ J("section", {
					class: "card",
					children: [
						/* @__PURE__ */ J("h2", { children: "Browser connection" }),
						/* @__PURE__ */ J("p", { children: e.client.views.ready ? "Live" : "Stale · reconnecting" }),
						/* @__PURE__ */ J("p", {
							class: "hint",
							children: [e.client.lost, " input actions reported lost"]
						})
					]
				}),
				/* @__PURE__ */ J("section", {
					class: "card",
					children: [/* @__PURE__ */ J("h2", { children: "MQTT" }), /* @__PURE__ */ J("p", {
						class: "muted",
						children: "Connection status is not exposed by this build."
					})]
				})
			]
		}),
		/* @__PURE__ */ J("section", {
			class: "card settings-section",
			children: [/* @__PURE__ */ J("h2", { children: "Recent commands" }), e.client.jobs.length ? /* @__PURE__ */ J("div", {
				class: "job-list",
				children: e.client.jobs.map((e) => /* @__PURE__ */ J("article", { children: [
					/* @__PURE__ */ J("strong", { children: e.command }),
					/* @__PURE__ */ J("span", {
						class: "pill unknown",
						children: e.status
					}),
					/* @__PURE__ */ J("p", {
						class: "hint",
						children: [
							e.target ?? "Controller",
							" · ",
							e.id
						]
					}),
					e.error && /* @__PURE__ */ J("p", {
						class: "error",
						children: e.error.message ?? e.error.code
					})
				] }, e.id))
			}) : /* @__PURE__ */ J("p", {
				class: "muted",
				children: "No commands recorded in this bridge session."
			})]
		}),
		/* @__PURE__ */ J("section", {
			class: "card settings-section",
			children: [/* @__PURE__ */ J("h2", { children: "Recent input actions" }), e.client.events.length ? /* @__PURE__ */ J("div", {
				class: "job-list",
				children: e.client.events.map((e) => /* @__PURE__ */ J("article", { children: [/* @__PURE__ */ J("strong", { children: e.target }), /* @__PURE__ */ J("pre", { children: JSON.stringify(e, null, 2) })] }))
			}) : /* @__PURE__ */ J("p", {
				class: "muted",
				children: "Operate an input after discovery to see its actions here."
			})]
		})
	] });
}
function Zt() {
	let [e] = H(() => new Re()), [, t] = H(0), [n, r] = H(location.pathname), [i, a] = H(!1);
	U(() => {
		let n = e.client.subscribe(() => t((e) => e + 1));
		e.client.start().catch((t) => {
			e.client.error = t.message, e.touch();
		});
		let i = () => {
			r(location.pathname), a(!1);
		}, o = () => e.client.activity();
		return addEventListener("popstate", i), addEventListener("pointerdown", o), addEventListener("keydown", o), () => {
			n(), e.drafts.clear(), e.client.close(), removeEventListener("popstate", i), removeEventListener("pointerdown", o), removeEventListener("keydown", o);
		};
	}, []);
	let o = e.client.snapshot, s = /^\/buses\/([0-3])(\/groups|\/scenes)?$/.exec(n);
	return /* @__PURE__ */ J(Be.Provider, {
		value: e,
		children: /* @__PURE__ */ J("div", {
			class: "layout",
			children: [/* @__PURE__ */ J("aside", {
				class: `sidebar ${i ? "open" : ""}`,
				children: [
					/* @__PURE__ */ J("div", {
						class: "brand",
						children: [
							/* @__PURE__ */ J("span", {
								class: "brand-icon",
								children: "D"
							}),
							/* @__PURE__ */ J("div", { children: [/* @__PURE__ */ J("strong", { children: "DALI Bridge" }), /* @__PURE__ */ J("span", { children: o?.identity.controller ?? "Connecting…" })] }),
							/* @__PURE__ */ J("button", {
								class: "nav-close",
								"aria-label": "Close navigation",
								onClick: () => a(!1),
								children: "×"
							})
						]
					}),
					/* @__PURE__ */ J("p", {
						class: "nav-label",
						children: "CONTROLLER"
					}),
					/* @__PURE__ */ J(X, {
						class: n === "/" ? "selected" : "",
						href: "/",
						children: "Overview"
					}),
					/* @__PURE__ */ J("p", {
						class: "nav-label",
						children: "DALI BUSES"
					}),
					o?.inventory.buses.map((e) => /* @__PURE__ */ J(X, {
						class: s && Number(s[1]) === e.bus ? "selected" : "",
						href: `/buses/${e.bus}`,
						children: [/* @__PURE__ */ J("span", { children: e.name }), /* @__PURE__ */ J("span", {
							class: "bus-count",
							children: e.outputs.length
						})]
					})),
					/* @__PURE__ */ J("div", {
						class: "sidebar-bottom",
						children: [
							/* @__PURE__ */ J(X, {
								class: n === "/settings" ? "selected" : "",
								href: "/settings",
								children: "Settings"
							}),
							/* @__PURE__ */ J(X, {
								class: n === "/diagnostics" ? "selected" : "",
								href: "/diagnostics",
								children: "Diagnostics"
							}),
							/* @__PURE__ */ J("p", {
								class: "hint",
								children: [
									o?.identity.version ?? "DALI v2",
									/* @__PURE__ */ J("br", {}),
									o?.identity.build_utc
								]
							})
						]
					})
				]
			}), /* @__PURE__ */ J("div", {
				class: "main-shell",
				children: [
					/* @__PURE__ */ J("header", {
						class: "topbar",
						children: [
							/* @__PURE__ */ J("button", {
								class: "nav-toggle",
								"aria-label": "Open navigation",
								onClick: () => a(!0),
								children: "☰"
							}),
							/* @__PURE__ */ J("span", {
								class: "muted",
								children: o?.identity.controller ?? "DALI controller"
							}),
							/* @__PURE__ */ J("span", { class: "spacer" }),
							/* @__PURE__ */ J("span", {
								class: `pill ${e.client.views.ready ? "" : "unknown"}`,
								children: e.client.views.ready ? "Live" : "Connecting"
							}),
							/* @__PURE__ */ J("button", {
								disabled: e.signingOut,
								onClick: () => void e.logout(),
								children: e.signingOut ? "Signing out…" : "Sign out"
							})
						]
					}),
					/* @__PURE__ */ J("main", { children: [
						(e.error || e.client.error) && /* @__PURE__ */ J("div", {
							class: "banner error",
							role: "alert",
							children: e.error || e.client.error
						}),
						e.notice && /* @__PURE__ */ J("div", {
							class: "banner notice",
							role: "status",
							children: e.notice
						}),
						o ? s ? /* @__PURE__ */ J(Jt, {
							number: Number(s[1]),
							tab: s[2] ?? ""
						}, s[1]) : n === "/settings" ? /* @__PURE__ */ J(Yt, {}) : n === "/diagnostics" ? /* @__PURE__ */ J(Xt, {}) : n === "/" ? /* @__PURE__ */ J(qt, {}) : /* @__PURE__ */ J("div", {
							class: "card",
							children: [/* @__PURE__ */ J("h1", { children: "Page not found" }), /* @__PURE__ */ J(X, {
								href: "/",
								children: "Return to overview"
							})]
						}) : /* @__PURE__ */ J("div", {
							class: "card",
							children: [/* @__PURE__ */ J("h1", { children: "Connecting to the controller…" }), /* @__PURE__ */ J("p", {
								class: "muted",
								children: "Waiting for the live inventory."
							})]
						})
					] }),
					/* @__PURE__ */ J("footer", { children: [
						o?.identity.controller,
						" · ",
						o?.identity.version,
						" · build ",
						o?.identity.build_utc
					] })
				]
			})]
		})
	});
}
I(/* @__PURE__ */ J(Zt, {}), document.getElementById("app"));
