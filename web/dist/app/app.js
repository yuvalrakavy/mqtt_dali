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
		for (r = 1; n.length;) n.length > r && n.sort(a), t = n.shift(), r = n.length, 8 & t.__g && (s = void 0, c = void 0, l = (c = (o = t).__v).__e, u = [], d = [], (f = o.__P) && ((s = g({ constructor: void 0 }, c)).__v = c.__v + 1, e.vnode && e.vnode(s), j(f, s, c, o.__n, f.namespaceURI, 32 & c.__u ? [l] : null, u, l || x(c), 32 & c.__u, d), s.__v = c.__v, s.__.__k[s.__i] = s, M(u, s, d), c.__ = c.__e = null, s.__e != l && S(s)));
	} finally {
		n.length = i = 0;
	}
}
function ee(e, t, n, r, i, a, o, s, c, l, u) {
	var p, m, h, g, _, v, y = r.__k || f, b = t.length;
	for (c = T(n, t, y, c, b), p = 0; p < b; p++) (h = n.__k[p]) != null && (m = ~h.__i && y[h.__i] || d, h.__i = p, v = j(e, h, m, i, a, o, s, c, l, u), g = h.__e, (m.ref != h.ref || 8 & m.__u) && (m.ref && N(m.ref, null, h, m), h.ref && u.push(h.ref, h.__c || g, h)), _ = _ || g, 4 & h.__u ? (c = E(h, c, e, !m.__v), m.__e && (m.__e = null)) : typeof h.type == "function" && v !== void 0 ? c = v : g && (c = g.nextSibling), h.__u &= -7);
	return n.__e = _, c;
}
function T(e, t, n, r, i) {
	var a, o, s, c, l, u, d, f, p, h, g = n.length, _ = g, b = 0, S = !1, C = e.__k = Array(i);
	for (a = 0; a < i; a++) (o = t[a]) != null && typeof o != "boolean" && typeof o != "function" ? (typeof o != "object" || o.constructor == String ? o = C[a] = v(null, o) : m(o) ? o = C[a] = v(y, { children: o }) : o.constructor === void 0 && o.__b ? o = C[a] = v(o.type, o.props, o.key, o.ref, o.__v) : C[a] = o, c = a + b, o.__ = e, o.__b = e.__b + 1, s = null, ~(l = o.__i = D(o, n, c, _)) && (_--, (s = n[l]) && (s.__u |= 2)), s && s.__v ? (o.__u |= 2, l == c - 1 ? b-- : l == c + 1 ? b++ : l != c && (l > c ? b-- : b++, S = !0)) : (~l || (i > g ? b-- : i < g && b++), typeof o.type != "function" && (o.__u |= 4))) : C[a] = null;
	if (S) {
		for (u = [], d = [], a = 0; a < i; a++) if ((o = C[a]) && 2 & o.__u) {
			for (f = 0, p = u.length; f < p;) u[h = f + p >> 1] < o.__i ? f = h + 1 : p = h;
			u[f] = o.__i, d[a] = f + 1;
		}
		for (b = u.length; a--;) d[a] && (d[a] == b ? b-- : C[a].__u |= 4);
	}
	if (_) for (a = 0; a < g; a++) !(s = n[a]) || 2 & s.__u || (s.__e == r && (r = x(s)), P(s, s));
	return r;
}
function E(e, t, n, r) {
	var i, a, o;
	if (typeof e.type == "function") {
		if (e.props.__P) return t;
		if (i = e.__k) for (a = 0; a < i.length; a++) i[a] && (i[a].__ = e, t = E(i[a], t, n, !1));
		return t;
	}
	for (t && !t.parentNode && (t = x(e)) && !t.parentNode && (t = null), o = t; o && o.nodeType == 8;) o = o.nextSibling;
	for (e.__e != o && (!r && n.moveBefore && e.__e.parentNode ? n.moveBefore(e.__e, t) : n.insertBefore(e.__e, t || null)), t = e.__e; (t = t && t.nextSibling) && t.nodeType == 8;);
	return t;
}
function D(e, t, n, r) {
	var i, a, o, s = e.key, c = e.type, l = t[n], u = l && !(2 & l.__u);
	if (l === null && s == null || u && s == l.key && c == l.type) return n;
	if (r > +!!u) {
		for (i = n - 1, a = n + 1; i >= 0 || a < t.length;) if ((l = t[o = i >= 0 ? i-- : a++]) && !(2 & l.__u) && s == l.key && c == l.type) return o;
	}
	return -1;
}
function O(e, t, n) {
	n ?? (n = ""), t[0] == "-" ? e.setProperty(t, n) : e[t] = n;
}
function k(e, t, n, r, i) {
	var a, o;
	n: if (t == "style") {
		if (typeof n == "string") e.style.cssText = n;
		else {
			if (typeof r == "string" && (e.style.cssText = r = ""), r) for (t in r) n && t in n || O(e.style, t, "");
			if (n) for (t in n) r && n[t] == r[t] || O(e.style, t, n[t]);
		}
	} else if (t[0] == "o" && t[1] == "n") (e.__e || (e.__e = {}))[t] = n, n && r || (o = l[t] || (l[t] = A(t)), (e.__a || (e.__a = {}))[t] = c, a = t != (t = t.replace(s, "$1")), (t = t.slice(2))[0] < "a" && (t = t.toLowerCase()), n ? e.addEventListener(t, o, a) : e.removeEventListener(t, o, a));
	else {
		if (i == "http://www.w3.org/2000/svg") t = t.replace(/xlink(H|:h)/, "h").replace(/sName$/, "s");
		else if (t != "width" && t != "height" && t != "href" && t != "list" && t != "form" && t != "tabIndex" && t != "download" && t != "rowSpan" && t != "colSpan" && t != "role" && t != "popover" && t in e) try {
			e[t] = n ?? "";
			break n;
		} catch {}
		typeof n == "function" || (n == null || !1 === n && t[4] != "-" ? e.removeAttribute(t) : e.setAttribute(t, t == "popover" && n == 1 ? "" : n));
	}
}
function A(t) {
	return function(n) {
		if (this.__e) {
			var r = this.__e[t];
			if (n[o] == null) n[o] = c++;
			else if (n[o] < this.__a[t]) return;
			return r(e.event ? e.event(n) : n);
		}
	};
}
function j(t, n, r, i, a, o, s, c, l, u) {
	var d, p, h, v, S, C, w, T, E, D, O, k, A, j, M, N, F, I, L, R, z = n.type;
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
	n: if (typeof z == "function") {
		S = s.length;
		try {
			if (D = n.props, O = (d = z.prototype) && d.render, k = (d = z.contextType) && i[d.__c], A = d ? k ? k.props.value : d.__ : i, r.__c ? 2 & (C = n.__c = r.__c).__g && (C.__g |= 1) : (O ? n.__c = C = new z(D, A) : (n.__c = C = new b(D, A), C.constructor = z, C.render = re), k && k.sub(C), C.state || (C.state = {}), C.__n = i, C.__g |= 8, C.__h = [], C.__k = []), O && (C.__s || (C.__s = C.state), z.getDerivedStateFromProps && (C.__s == C.state && (C.__s = g({}, C.__s)), g(C.__s, z.getDerivedStateFromProps(D, C.__s)))), w = C.props, T = C.state, C.__v = n, r.__c) {
				if (O && !z.getDerivedStateFromProps && D !== w && C.componentWillReceiveProps && C.componentWillReceiveProps(D, A), n.__v == r.__v && !(8 & C.__g) || !(4 & C.__g) && C.shouldComponentUpdate && !1 === C.shouldComponentUpdate(D, C.__s, A)) {
					n.__v != r.__v && (C.props = D, C.state = C.__s, C.__g &= -9), n.__e = r.__e, n.__k = r.__k, n.__k.some(function(e) {
						e && (e.__ = n);
					}), f.push.apply(C.__h, C.__k), C.__k = [], C.__h.length && s.push(C), c = x(r);
					break n;
				}
				C.componentWillUpdate && C.componentWillUpdate(D, C.__s, A), O && C.componentDidUpdate && C.__h.push(function() {
					C.componentDidUpdate(w, T, E);
				});
			} else O && !z.getDerivedStateFromProps && C.componentWillMount && C.componentWillMount(), O && C.componentDidMount && C.__h.push(C.componentDidMount);
			if (C.context = A, C.props = D, C.__P = t, C.__g &= -5, j = e.__r, M = 0, O) C.state = C.__s, C.__g &= -9, j && j(n), d = C.render(C.props, C.state, C.context), f.push.apply(C.__h, C.__k), C.__k = [];
			else do
				C.__g &= -9, j && j(n), d = C.render(C.props, C.state, C.context), C.state = C.__s;
			while (8 & C.__g && ++M < 25);
			C.state = C.__s, C.getChildContext && (i = g({}, i, C.getChildContext())), O && r.__c && C.getSnapshotBeforeUpdate && (E = C.getSnapshotBeforeUpdate(w, T)), N = d && d.type === y && d.key == null ? d.props.children : d, D.__P && (d = c, a = (t = D.__P).namespaceURI, l = o = null, r.props && r.props.__P != t && (r.__k.some(function(e) {
				e && P(e, e);
			}), r.__k = null), c = r.__k ? x(r, 0) : null), c = ee(t, m(N) ? N : [N], n, r, i, a, o, s, c, l, u), D.__P && (n.__e = null, c = d), n.__u &= -161, 128 & r.__u && (C.__z = null), p && p.some(_), C.__h.length && s.push(C), 1 & C.__g && (C.__g &= -4);
		} catch (t) {
			if (s.length = S, n.__v = null, l || o) {
				if (t.then) {
					if (F = 0, n.__u |= l ? 160 : 128, o) {
						for (~(L = o.indexOf(c || void 0)) || (L = o.length); (R = o[L - 1]) && R.nodeType == 8;) L--;
						for (; L < o.length; L++) if (R = o[L]) {
							if (o[L] = null, R.nodeType == 8) {
								if (R.data.startsWith("$s")) F++ || (I = R);
								else if (F && R.data.startsWith("/$s") && !--F) {
									c = R;
									break;
								}
							} else if (!F) break;
						}
					}
					if (!I) {
						for (; c && c.nodeType == 8 && c.nextSibling;) c = c.nextSibling;
						I = c;
					}
					n.__c.__z || (n.__c.__z = I), n.__e = c;
				} else o && o.some(_);
			} else n.__e = r.__e;
			n.__k || (n.__k = r.__k || []), t.then || te(n), e.__e(t, n, r);
		}
	} else c = n.__e = ne(r.__e, n, r, i, a, o, s, l, u, t);
	return (d = e.diffed) && d(n), 128 & n.__u ? void 0 : c;
}
function te(e) {
	e && (e.__c && (e.__c.__g |= 4), e.__k && e.__k.some(te));
}
function M(t, n, r) {
	for (var i = 0; i < r.length;) N(r[i++], r[i++], r[i++]);
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
function ne(t, n, r, i, a, o, s, c, l, u) {
	var f, g, v, y, b, S, C, w, T, E = r.props || d, D = n.props, O = n.type;
	if (O == "svg" ? a = "http://www.w3.org/2000/svg" : O == "math" ? a = "http://www.w3.org/1998/Math/MathML" : a || (a = "http://www.w3.org/1999/xhtml"), o) {
		for (f = 0; f < o.length; f++) if ((b = o[f]) && (O ? b.localName == O : b.nodeType == 3)) {
			t = b, o[f] = null;
			break;
		}
	}
	if (!t) {
		if (w = u.ownerDocument || document, !O) return w.createTextNode(D);
		t = w.createElementNS(a, O, D.is && D), c && (e.__m && e.__m(n, o), c = !1), o = null;
	}
	if (O) {
		if (u = O == "template" ? t.content : t, o = O == "textarea" && D.defaultValue != null ? null : o && h.call(u.childNodes), !c && o) for (E = {}, f = 0; f < t.attributes.length; f++) E[(b = t.attributes[f]).name] = b.value;
		for (f in E) b = E[f], f == "dangerouslySetInnerHTML" ? v = b : f == "children" || f in D || f == "value" && "defaultValue" in D || f == "checked" && "defaultChecked" in D || k(t, f, null, b, a);
		for (f in T = 1 & r.__u, D) b = D[f], f == "children" ? y = b : f == "dangerouslySetInnerHTML" ? g = b : f == "value" ? S = b : f == "checked" ? C = b : c && typeof b != "function" || !(E[f] !== b || T && b != null) || k(t, f, b, E[f], a);
		g ? (c || v && (g.__html == v.__html || g.__html == t.innerHTML) || (t.innerHTML = g.__html), n.__k = []) : (v && (t.textContent = ""), (O == "foreignObject" || a == "http://www.w3.org/1998/Math/MathML" && p.test(O)) && (a = "http://www.w3.org/1999/xhtml"), ee(u, m(y) ? y : [y], n, r, i, a, o, s, o ? o[0] : r.__k && x(r, 0), c, l), o && o.some(_)), c && O != "textarea" || (f = "value", O == "progress" && S == null ? t.removeAttribute(f) : S == null || S === t[f] && (O != "progress" || S) || k(t, f, S, E[f], a), f = "checked", C != null && C != t[f] && k(t, f, C, E[f], a));
	} else E === D || c && t.data == D || (t.data = D);
	return t;
}
function N(t, n, r, i) {
	try {
		typeof t == "function" ? n ? n.__x = t(n) || 1 : i && (i = i.__c || i.__e) && (n = i.__x) && (i.__x = null, typeof n == "function" ? n() : t(null)) : t.current = n;
	} catch (t) {
		e.__e(t, r);
	}
}
function P(t, n, r) {
	var i, a;
	if (e.unmount && e.unmount(t), !(i = t.ref) || i.current && i.current != t.__e || N(i, null, n, t), i = t.__c) {
		if (i.componentWillUnmount) try {
			i.componentWillUnmount();
		} catch (t) {
			e.__e(t, n);
		}
		i.__P = i.__n = null;
	}
	if (i = t.__k) for (a = 0; a < i.length; a++) i[a] && P(i[a], n, typeof t.type != "function" || r && !t.props.__P);
	(i = t.__e) && (r || _(i), i.__e && (i.__e = null)), t.__e = t.__c = t.__ = null;
}
function re(e, t, n) {
	return this.constructor(e, n);
}
function F(t, n) {
	var r, i, a, o;
	e.__ && e.__(t, n), n.nodeType == 9 && (n = n.documentElement), i = (r = t && 32 & t.__u) ? null : n.__k, n.__k = v(y, { children: [t] }), a = [], o = [], j(n, n.__k, i || d, d, n.namespaceURI, i ? null : n.firstChild ? h.call(n.childNodes) : null, a, i ? i.__e : n.firstChild, r, o), M(a, n.__k, o), n.__k.props.children = null;
}
function I(e) {
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
var L, R, z, ie, B = Object.is, V = 0, ae = [], H = [], U = e, oe = U.__b, se = U.__r, ce = U.diffed, le = U.__c, ue = U.unmount, de = U.__;
function W(e, t) {
	U.__h && U.__h(R, e, V || t), V = 0;
	var n = R.__H || (R.__H = {
		__: [],
		__h: []
	});
	return e >= n.__.length && n.__.push({}), n.__[e];
}
function G(e) {
	return V = 1, fe(Ce, e);
}
function fe(e, t, n) {
	var r = W(L++, 2);
	if (r.t = e, !r.__c && (r.__ = [n ? n(t) : Ce(void 0, t), function(e) {
		var t = r.__N ? r.__N[0] : r.__[0], n = r.t(t, e);
		B(t, n) || (r.__N = [n, r.__[1]], r.__c.setState({}));
	}], r.__c = R, !R.__f)) {
		R.__f = !0;
		var i = R.shouldComponentUpdate;
		R.shouldComponentUpdate = function(e, t, n) {
			var r = this.__H;
			if (!r) return !0;
			var a = !1, o = this.props != e;
			if (r.__.some(function(e) {
				e.__N && (a = !0, B(e.__[0], e.__N[0]) || (o = !0));
			}), i) {
				var s = i.call(this, e, t, n);
				return a ? s || o : s;
			}
			return !a || o;
		};
	}
	return r.__;
}
function pe(e, t) {
	var n = W(L++, 3);
	!U.__s && Se(n.__H, t) && (n.__P = !0, n.__ = e, n.u = t, R.__H.__h.push(n));
}
function me(e) {
	return V = 5, he(function() {
		return { current: e };
	}, []);
}
function he(e, t) {
	var n = W(L++, 7);
	return Se(n.__H, t) && (n.__ = e(), n.__H = t), n.__;
}
function ge(e) {
	var t = R.context[e.__c], n = W(L++, 9);
	return n.c = e, t ? (n.__ ?? (n.__ = !0, t.sub(R)), t.props.value) : e.__;
}
function _e() {
	var e;
	do {
		for (; e = H.shift();) try {
			K(e);
		} catch (t) {
			U.__e(t, { __: (e = e.__P) && e.__v });
		}
		for (; e = ae.shift();) {
			var t = e.__H;
			if (e.__P && t) try {
				t.__h.some(K), t.__h.some(xe), t.__h = [];
			} catch (n) {
				t.__h = [], U.__e(n, e.__v);
			}
		}
	} while (H.length);
}
U.__b = function(e) {
	R = null, oe && oe(e);
}, U.__ = function(e, t) {
	e && t.__k && t.__k.__m && (e.__m = t.__k.__m), de && de(e, t);
}, U.__r = function(e) {
	se && se(e), L = 0;
	var t = (R = e.__c).__H;
	t && (z == R ? R.__h = [] : (t.__h.some(K), t.__h.some(xe), L = 0), t.__h = [], t.__.some(function(e) {
		e.__N && (e.__ = e.__N), e.u = e.__N = void 0;
	})), z = R;
}, U.diffed = function(e) {
	ce && ce(e);
	var t = e.__c;
	t && t.__H && (t.__H.__h.length && be(ae.push(t)), t.__H.__.some(function(e) {
		e.u && (e.__H = e.u);
	})), z = R = null;
}, U.__c = function(e, t) {
	t.some(function(e) {
		try {
			e.__h.some(K), e.__h = e.__h.filter(function(e) {
				return !e.__ || xe(e);
			});
		} catch (n) {
			t.some(function(e) {
				e.__h && (e.__h = []);
			}), t = [], U.__e(n, e.__v);
		}
	}), le && le(e, t);
}, U.unmount = function(e) {
	ue && ue(e);
	var t, n, r = e.__c;
	r && r.__H && (r.__H.__.some(function(r) {
		try {
			if (r.__P && r.__c) {
				if (n === void 0) {
					for (n = e.__; n && (!n.__c || !n.__c.__P);) n = n.__;
					n = n && n.__c;
				}
				r.__P = n, be(H.push(r));
			} else K(r);
		} catch (e) {
			t = e;
		}
	}), r.__H = void 0, t && U.__e(t, r.__v));
};
var ve = typeof requestAnimationFrame == "function";
function ye(e) {
	var t, n = function() {
		clearTimeout(r), ve && cancelAnimationFrame(t), setTimeout(e);
	}, r = setTimeout(n, 35);
	ve && (t = requestAnimationFrame(n));
}
function be(e) {
	e != 1 && ie == U.requestAnimationFrame || ((ie = U.requestAnimationFrame) || ye)(_e);
}
function K(e) {
	var t = R, n = e.__c;
	typeof n == "function" && (e.__c = void 0, n()), R = t;
}
function xe(e) {
	var t = R;
	e.__c = e.__(), R = t;
}
function Se(e, t) {
	return !e || e.length != t.length || t.some(function(t, n) {
		return !B(t, e[n]);
	});
}
function Ce(e, t) {
	return typeof t == "function" ? t(e) : t;
}
// src/model.ts
function q(e, t) {
	if (!/^[a-f0-9]{32}$/.test(e) || !Number.isSafeInteger(t) || t < 1) throw Error("Invalid inventory authority; refresh the controller.");
	return `${e}.${t}`;
}
function we(e, t) {
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
function Te(e) {
	let t = e.inventory;
	q(t.inventory_epoch, t.revision);
	let n = we(t.identity_ledger, t.inventory_epoch), r = [];
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
				generation: q(t.inventory_epoch, a.binding_revision)
			});
		};
		for (let t of e.outputs) i("output", t);
		for (let t of e.groups ?? []) i("group", t);
		for (let t of e.inputs ?? []) {
			i("input", t);
			for (let e of t.instances ?? []) i("instance", e, t);
		}
	}
	return r;
}
function Ee(e, t) {
	if (!e) return !0;
	let n = e.replace(/[.+^${}()|[\]\\]/g, "\\$&").replace(/\*/g, ".*").replace(/\?/g, ".");
	return RegExp(`^${n}$`, "iu").test(t);
}
function J(e, t) {
	return e.phase === "ready" && t.enabled && e.hardware.state === "ready" && !!e.hardware.buses?.some((e) => e.bus === t.bus && e.status === 2);
}
function De(e, t) {
	return e.states.find((e) => e.target === t.item.target && e.generation === t.generation)?.values ?? {};
}
var Oe = (e) => ({
	1: "Part 301 · button",
	2: "Part 302 · input",
	3: "Part 303 · occupancy",
	4: "Part 304 · light sensor"
})[e ?? 0] ?? `Type ${e ?? "unknown"}`;
function ke(e) {
	return typeof e.Status == "boolean" ? e.Status ? "Pressed" : "Released" : typeof e.InputState == "boolean" ? e.InputState ? "On" : "Off" : typeof e.Occupied == "boolean" ? e.Occupied ? "Occupied" : "Vacant" : typeof e.InputValueRaw == "string" ? `${typeof e.Position == "number" ? `${Math.round(e.Position * 100)}% · ` : ""}0x${e.InputValueRaw} / ${e.ValueResolution} bit` : typeof e.PositionRaw == "number" ? `${e.PositionRaw} raw` : typeof e.LightLevelRaw == "number" ? `${e.LightLevelRaw} raw` : "Unknown";
}
var Ae = class {
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
}, je = class {
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
function Me(e) {
	let t = {
		...e,
		inventory: e.inventory.Inventory
	};
	return Te(t), t;
}
function Y(e, t, n, r) {
	let i = e.inventory.buses.find((e) => e.bus === r), a = t.inventory.buses.find((e) => e.bus === r);
	return e.inventory.inventory_epoch === t.inventory.inventory_epoch && t.inventory.revision === n && !!i && !!a && i.name === a.name && a.enabled;
}
var Ne = class extends Error {
	constructor(e, t) {
		super(t), this.status = e;
	}
}, Pe = class {
	constructor() {
		this.views = new je(), this.csrf = "", this.error = "", this.events = [], this.lost = 0, this.stopped = !1, this.session = 0, this.activeAt = 0, this.requests = /* @__PURE__ */ new Set(), this.sleepers = /* @__PURE__ */ new Map(), this.listeners = /* @__PURE__ */ new Set();
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
			if (i.status === 401) throw this.expired(), new Ne(401, "Sign in required.");
			let a = i.status === 204 ? null : await i.json();
			if (n !== this.session || this.stopped) throw Error("Session ended.");
			if (!i.ok) throw new Ne(i.status, a?.error ?? "The controller refused this request.");
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
				this.views.snapshot(e, Me(JSON.parse(t.data))), this.error = "", this.notify();
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
			if (e && e.status !== "running" || (e = await this.request(`/api/jobs/${a}`)), e?.status === "succeeded") return e.result;
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
}, Fe = class {
	constructor() {
		this.client = new Pe(), this.drafts = /* @__PURE__ */ new Map(), this.hints = /* @__PURE__ */ new Map(), this.error = "", this.notice = "", this.busy = 0, this.progress = "", this.cancelled = !1, this.signingOut = !1, this.edits = Promise.resolve();
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
		return Te(this.snapshot);
	}
	current(e) {
		let t = this.currentRows.find((t) => t.key === e);
		if (!t) throw Error("This fixture was removed or replaced.");
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
		await this.metadata("configure_bus", null, q(r.inventory.inventory_epoch, r.inventory.revision), {
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
			if (!Y(r, this.snapshot, n, e.bus)) throw Error("Bus configuration changed during discovery. Review before starting again.");
			let a = await this.client.job(q(r.inventory.inventory_epoch, n), null, t, {
				...i,
				expected_revision: n
			});
			if (!Number.isSafeInteger(a.revision)) throw Error("The controller returned an invalid inventory revision.");
			let o = await this.changed(a.revision);
			if (n = a.revision, !Y(r, o, n, e.bus)) throw Error("Configuration changed during discovery. Completed steps remain saved.");
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
				if (!Y(r, this.snapshot, n, e.bus)) throw Error("Configuration changed before refresh.");
				this.progress = `Reading ${t.item.name}`, this.touch(), await this.read(t);
			}
			this.notice = this.cancelled ? "Stopped after the current operation. Completed changes remain saved." : "Discovery and current-value refresh finished.";
		} finally {
			this.progress = "", this.touch();
		}
	}
	async createGroup(e, t, n) {
		let r = this.snapshot;
		await this.metadata("create_group", null, q(r.inventory.inventory_epoch, r.inventory.revision), {
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
}, Ie = 0;
Array.isArray;
function X(t, n, r, i, a, o) {
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
		__v: --Ie,
		__i: -1,
		__u: 0
	};
	return (a || o) && (u.__source = a, u.__self = o), e.vnode && e.vnode(u), u;
}
// src/ui.tsx
var Le = I(null);
function Z() {
	let e = ge(Le);
	if (!e) throw Error("Controller is unavailable.");
	return e;
}
function Re() {
	let e = Z();
	return e.progress ? /* @__PURE__ */ X("div", {
		class: "progress",
		role: "status",
		children: [/* @__PURE__ */ X("span", { children: e.progress }), /* @__PURE__ */ X("button", {
			onClick: () => {
				e.cancelled = !0, e.touch();
			},
			disabled: e.cancelled,
			children: e.cancelled ? "Stopping…" : "Stop after current step"
		})]
	}) : null;
}
function ze(e) {
	history.pushState(null, "", e), dispatchEvent(new PopStateEvent("popstate"));
}
function Q(e) {
	return /* @__PURE__ */ X("a", {
		href: e.href,
		class: e.class,
		onClick: (t) => {
			t.metaKey || t.ctrlKey || t.shiftKey || t.altKey || (t.preventDefault(), ze(e.href));
		},
		children: e.children
	});
}
function $(e) {
	let t = Z(), n = me(null);
	return pe(() => (n.current?.showModal(), () => n.current?.close()), []), /* @__PURE__ */ X("dialog", {
		ref: n,
		onCancel: (t) => {
			t.preventDefault(), e.close();
		},
		onClick: (t) => {
			t.target === t.currentTarget && e.close();
		},
		children: /* @__PURE__ */ X("div", {
			class: "dialog-body",
			children: [
				/* @__PURE__ */ X("header", { children: [/* @__PURE__ */ X("h2", { children: e.title }), /* @__PURE__ */ X("button", {
					type: "button",
					onClick: e.close,
					"aria-label": "Close dialog",
					children: "×"
				})] }),
				e.children,
				t.error && /* @__PURE__ */ X("p", {
					class: "error",
					role: "alert",
					children: t.error
				})
			]
		})
	});
}
function Be({ row: e }) {
	let t = Z(), n = t.drafts.get(e.key), r = () => {
		let n = t.drafts.get(e.key);
		n && (n.commit((n, r) => t.rename(e.key, n, r)).catch(() => {}).finally(() => t.touch()), t.touch());
	}, i = me(null);
	return /* @__PURE__ */ X("div", {
		class: "name-cell",
		children: [
			/* @__PURE__ */ X("div", {
				class: `name-edit ${n?.dirty ? "dirty" : ""}`,
				children: [/* @__PURE__ */ X("input", {
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
						r || (r = new Ae(e.item.name, e.item.target, e.generation), t.drafts.set(e.key, r)), r.text = n.currentTarget.value, r.error = "", t.touch();
					},
					onBlur: r,
					onKeyDown: (n) => {
						n.key === "Enter" && (n.preventDefault(), r()), n.key === "Escape" && (t.drafts.delete(e.key), t.touch(), i.current?.blur());
					}
				}), n?.dirty && /* @__PURE__ */ X("button", {
					type: "button",
					"aria-label": `Save name ${e.item.name}`,
					disabled: !!n.pending,
					onPointerDown: (e) => e.preventDefault(),
					onClick: r,
					children: "✓"
				})]
			}),
			n?.pending && /* @__PURE__ */ X("p", {
				class: "hint",
				role: "status",
				children: "Saving…"
			}),
			n?.error && /* @__PURE__ */ X("p", {
				class: "error",
				role: "alert",
				children: [
					n.error,
					" ",
					/* @__PURE__ */ X("button", {
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
// src/devices.tsx
function Ve({ row: e, close: t }) {
	let n = Z(), r = n.current(e.key), i = De(n.snapshot, r), a = r.item.color_representation ?? "", o = i.ColorCapabilities, s = o?.supported_representations, c = o?.temperature_limits, l = a === "temperature" ? ["ColorTemperature"] : a === "xy" ? ["ColorX", "ColorY"] : a === "rgbwaf" ? [
		"ColorRed",
		"ColorGreen",
		"ColorBlue",
		"ColorWhite",
		"ColorAmber",
		"ColorFree"
	] : [], [u, d] = G({}), f = (e) => ({
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
	return /* @__PURE__ */ X($, {
		title: `Colour · ${r.item.name}`,
		close: t,
		children: [
			/* @__PURE__ */ X("label", {
				class: "field",
				children: ["Representation", /* @__PURE__ */ X("select", {
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
						/* @__PURE__ */ X("option", {
							value: "",
							children: "None selected"
						}),
						/* @__PURE__ */ X("option", {
							value: "temperature",
							children: "White temperature"
						}),
						/* @__PURE__ */ X("option", {
							value: "xy",
							children: "CIE xy"
						}),
						/* @__PURE__ */ X("option", {
							value: "rgbwaf",
							children: "RGBWAF channels"
						})
					]
				})]
			}),
			/* @__PURE__ */ X("p", {
				class: "hint",
				children: "Choose the values this fixture uses. Selecting a representation does not change its physical operating mode."
			}),
			/* @__PURE__ */ X("p", {
				class: "hint",
				children: [
					"Observed support: ",
					s ? s.length ? s.join(", ") : "no supported colour representations" : "unknown",
					o?.rgbwaf_channel_count ? ` · ${o.rgbwaf_channel_count} RGBWAF channels` : "",
					c?.warmest_kelvin && c.coolest_kelvin ? ` · ${Math.round(c.warmest_kelvin)}–${Math.round(c.coolest_kelvin)} K` : ""
				]
			}),
			/* @__PURE__ */ X("div", {
				class: "colour-fields",
				children: l.map((e) => /* @__PURE__ */ X("label", {
					class: "field",
					children: [
						f(e),
						/* @__PURE__ */ X("input", {
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
						/* @__PURE__ */ X("span", {
							class: "hint",
							children: ["Actual: ", p(e, i[e]) || "unknown"]
						})
					]
				}, e))
			}),
			/* @__PURE__ */ X("p", {
				class: "hint",
				children: i.ColorQuality === void 0 ? "Read the fixture to discover colour support." : `Colour status: ${i.ColorQuality}`
			}),
			/* @__PURE__ */ X("div", {
				class: "dialog-actions",
				children: [/* @__PURE__ */ X("button", {
					disabled: !!n.busy,
					onClick: () => void n.run(() => n.read(r)),
					children: "Read values"
				}), /* @__PURE__ */ X("button", {
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
function He({ row: e, close: t }) {
	let n = Z(), r = n.current(e.key);
	return /* @__PURE__ */ X($, {
		title: `Groups · ${r.item.name}`,
		close: t,
		children: [/* @__PURE__ */ X("p", {
			class: "hint",
			children: "Saved membership. Verify groups to compare with hardware."
		}), /* @__PURE__ */ X("div", {
			class: "checklist",
			children: [(r.bus.groups ?? []).map((e) => /* @__PURE__ */ X("label", { children: [
				/* @__PURE__ */ X("input", {
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
				/* @__PURE__ */ X("span", {
					class: "hint",
					children: e.group_address
				})
			] }, e.group_address)), !r.bus.groups?.length && /* @__PURE__ */ X("p", { children: "No groups are saved. Open the Groups tab to create one." })]
		})]
	});
}
function Ue({ row: e }) {
	let t = Z(), n = t.snapshot, r = De(n, e), [i, a] = G(!1), [o, s] = G(!1), [c, l] = G(!1), [u, d] = G(!1), [f, p] = G(null), m = J(n, e.bus) && t.client.views.ready, h = n.identification?.target === e.item.target ? n.identification : null, g = !!h && [
		"starting",
		"blinking",
		"restoring"
	].includes(h.phase), _ = t.client.events.find((t) => t.target === e.item.target && t.generation === e.generation), v = t.hints.get(e.key), b = e.kind === "output" ? v?.length ? v.map((e) => `DT${e}`).join(", ") : "Output" : e.kind === "input" ? "Input module" : Oe(e.item.instance_type), x = ke(r), S = typeof r.Brightness == "number" ? Math.round(r.Brightness * 100) : null;
	return /* @__PURE__ */ X("div", {
		class: `device-row ${e.kind === "output" ? "output-grid" : "input-grid"} ${g ? "identifying" : ""} ${_ && Date.now() - _.received < 5e3 ? "active-input" : ""}`,
		"data-device-key": e.key,
		children: [
			/* @__PURE__ */ X("span", {
				class: "address",
				children: e.parent ? `${e.parent.short_address} / ${e.item.number}` : e.item.short_address
			}),
			/* @__PURE__ */ X(Be, { row: e }),
			/* @__PURE__ */ X("span", {
				class: "device-type",
				children: b
			}),
			e.kind === "output" ? /* @__PURE__ */ X(y, { children: [
				/* @__PURE__ */ X("button", {
					class: `power ${r.PowerState === !0 ? "on" : r.PowerState === !1 ? "off" : "unknown"}`,
					"aria-label": `Power ${e.item.name}`,
					"aria-pressed": typeof r.PowerState == "boolean" ? r.PowerState : "mixed",
					disabled: !m || !!t.busy || g,
					onClick: () => void t.run(() => t.apply(e, { PowerState: r.PowerState !== !0 })),
					children: r.PowerState === !0 ? "On" : r.PowerState === !1 ? "Off" : "?"
				}),
				/* @__PURE__ */ X("div", {
					class: "brightness",
					children: [/* @__PURE__ */ X("input", {
						type: "range",
						min: "0",
						max: "100",
						value: f ?? S ?? 0,
						"aria-label": `Brightness ${e.item.name}`,
						disabled: !m || !!t.busy || g || S === null,
						onInput: (e) => p(e.currentTarget.value),
						onChange: (n) => {
							let r = Number(n.currentTarget.value) / 100;
							t.run(async () => {
								try {
									await t.apply(e, { Brightness: r });
								} finally {
									p(null);
								}
							});
						}
					}), /* @__PURE__ */ X("output", { children: [f ?? S ?? "—", S === null ? "" : "%"] })]
				}),
				/* @__PURE__ */ X("button", {
					class: "colour-button",
					"aria-label": `Colour ${e.item.name}`,
					disabled: !m || !!t.busy || g,
					onClick: () => s(!0),
					children: e.item.color_representation === "temperature" && typeof r.ColorTemperature == "number" ? `${Math.round(r.ColorTemperature)} K` : e.item.color_representation === "xy" ? "xy controls" : e.item.color_representation === "rgbwaf" ? "RGB controls" : "Colour"
				}),
				/* @__PURE__ */ X("button", {
					class: g ? "primary blink" : "blink",
					"aria-label": `${g ? "Stop blink" : "Blink"} ${e.item.name}`,
					disabled: !m || !!t.busy || h?.phase === "restoring",
					onClick: () => void t.run(() => t.blink(e)),
					children: h?.phase === "restoring" ? "Restoring…" : g ? "Stop blink" : "Blink"
				})
			] }) : /* @__PURE__ */ X(y, { children: [/* @__PURE__ */ X("span", {
				class: `input-state ${r.Available === !0 ? "" : "muted"}`,
				children: e.kind === "input" ? `${e.item.instances?.length ?? 0} instances` : x
			}), /* @__PURE__ */ X("span", {
				class: "activity",
				children: _?.action ?? "No recent action"
			})] }),
			/* @__PURE__ */ X("button", {
				class: "menu-button",
				"aria-label": `Actions ${e.item.name}`,
				onClick: () => a(!0),
				children: "⋮"
			}),
			i && /* @__PURE__ */ X($, {
				title: e.item.name,
				close: () => a(!1),
				children: /* @__PURE__ */ X("div", {
					class: "menu-actions",
					children: [
						e.kind !== "input" && /* @__PURE__ */ X("button", {
							disabled: !m || !!t.busy,
							onClick: () => {
								a(!1), t.run(() => t.read(e));
							},
							children: "Read current values"
						}),
						e.kind === "output" && /* @__PURE__ */ X("button", {
							onClick: () => {
								a(!1), l(!0);
							},
							children: "Group membership"
						}),
						e.kind === "output" && /* @__PURE__ */ X("button", {
							onClick: () => {
								a(!1), s(!0);
							},
							children: "Colour representation and values"
						}),
						e.kind !== "instance" && /* @__PURE__ */ X("button", {
							class: "danger",
							onClick: () => {
								a(!1), d(!0);
							},
							children: "Remove from list"
						})
					]
				})
			}),
			o && /* @__PURE__ */ X(Ve, {
				row: e,
				close: () => s(!1)
			}),
			c && /* @__PURE__ */ X(He, {
				row: e,
				close: () => l(!1)
			}),
			u && /* @__PURE__ */ X($, {
				title: `Remove ${e.item.name}?`,
				close: () => d(!1),
				children: [/* @__PURE__ */ X("p", { children: [
					"Remove this ",
					e.kind === "input" ? "module and all its instances" : "fixture",
					" from the saved list. Its hardware address stays unchanged and discovery can find it again."
				] }), /* @__PURE__ */ X("div", {
					class: "dialog-actions",
					children: [/* @__PURE__ */ X("button", {
						onClick: () => d(!1),
						children: "Cancel"
					}), /* @__PURE__ */ X("button", {
						class: "danger",
						disabled: !!t.busy,
						onClick: () => void t.run(async () => {
							await t.remove(e), d(!1);
						}),
						children: "Remove from list"
					})]
				})]
			}),
			h?.phase === "interrupted" && /* @__PURE__ */ X("p", {
				class: "hint row-error",
				children: "Identification interrupted. Restoration was not confirmed."
			})
		]
	});
}
function We({ bus: e }) {
	let t = Z(), [n, r] = G(""), i = t.currentRows.filter((t) => t.bus.bus === e.bus), a = (e) => Ee(n, e.item.name) || !!t.drafts.get(e.key)?.dirty || !!t.drafts.get(e.key)?.error, o = i.filter((e) => e.kind === "output" && a(e)), s = i.filter((e) => (e.kind === "input" || e.kind === "instance") && a(e)), c = J(t.snapshot, e) && t.client.views.ready, l = [...t.drafts].filter(([e, n]) => n.dirty && !t.currentRows.some((t) => t.key === e));
	return /* @__PURE__ */ X(y, { children: [
		/* @__PURE__ */ X("div", {
			class: "toolbar discovery",
			children: [
				/* @__PURE__ */ X("button", {
					class: "primary",
					disabled: !c || !!t.busy,
					onClick: () => void t.run(() => t.scan(e, !0)),
					children: "Add unaddressed"
				}),
				/* @__PURE__ */ X("button", {
					disabled: !c || !!t.busy,
					onClick: () => void t.run(() => t.scan(e, !1)),
					children: "Refresh addressed"
				}),
				/* @__PURE__ */ X("span", {
					class: "hint",
					children: "Existing names and addresses are preserved."
				})
			]
		}),
		/* @__PURE__ */ X(Re, {}),
		/* @__PURE__ */ X("div", {
			class: "toolbar filter",
			children: [
				/* @__PURE__ */ X("input", {
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
				].map((e) => /* @__PURE__ */ X("button", {
					class: n === e ? "soft" : "",
					onClick: () => r(e),
					children: e || "All"
				})),
				/* @__PURE__ */ X("span", { class: "spacer" }),
				/* @__PURE__ */ X("span", {
					class: "hint",
					children: [
						i.filter((e) => e.kind === "output").length,
						" outputs · ",
						i.filter((e) => e.kind === "input").length,
						" input modules"
					]
				})
			]
		}),
		/* @__PURE__ */ X("div", {
			class: "section-heading",
			children: [/* @__PURE__ */ X("h2", { children: "Outputs" }), /* @__PURE__ */ X("span", {
				class: "hint",
				children: "✓, Enter or leave the field to save"
			})]
		}),
		/* @__PURE__ */ X("div", {
			class: "device-table",
			children: [
				/* @__PURE__ */ X("div", {
					class: "table-heading output-grid",
					children: [
						/* @__PURE__ */ X("span", { children: "Address" }),
						/* @__PURE__ */ X("span", { children: "Name" }),
						/* @__PURE__ */ X("span", { children: "Type" }),
						/* @__PURE__ */ X("span", { children: "Power" }),
						/* @__PURE__ */ X("span", { children: "Brightness" }),
						/* @__PURE__ */ X("span", { children: "Colour" }),
						/* @__PURE__ */ X("span", { children: "Identify" }),
						/* @__PURE__ */ X("span", {})
					]
				}),
				o.map((e) => /* @__PURE__ */ X(Ue, { row: e }, e.key)),
				!o.length && /* @__PURE__ */ X("p", {
					class: "empty",
					children: n ? "No matching outputs." : "No outputs are saved. Start discovery to find the fixtures on this bus."
				})
			]
		}),
		/* @__PURE__ */ X("div", {
			class: "section-heading",
			children: [/* @__PURE__ */ X("h2", { children: "Inputs" }), /* @__PURE__ */ X("span", {
				class: "hint",
				children: "Operate a switch to see its current state and action."
			})]
		}),
		/* @__PURE__ */ X("div", {
			class: "device-table",
			children: [
				/* @__PURE__ */ X("div", {
					class: "table-heading input-grid",
					children: [
						/* @__PURE__ */ X("span", { children: "Addr / instance" }),
						/* @__PURE__ */ X("span", { children: "Name" }),
						/* @__PURE__ */ X("span", { children: "Type" }),
						/* @__PURE__ */ X("span", { children: "Current state" }),
						/* @__PURE__ */ X("span", { children: "Last action" }),
						/* @__PURE__ */ X("span", {})
					]
				}),
				s.map((e) => /* @__PURE__ */ X(Ue, { row: e }, e.key)),
				!s.length && /* @__PURE__ */ X("p", {
					class: "empty",
					children: n ? "No matching inputs." : "No input modules are saved."
				})
			]
		}),
		l.map(([e, n]) => /* @__PURE__ */ X("div", {
			class: "card retired",
			children: [
				/* @__PURE__ */ X("p", { children: ["Removed or replaced fixture · unsaved name: ", /* @__PURE__ */ X("strong", { children: n.text })] }),
				/* @__PURE__ */ X("p", {
					class: "error",
					children: "This draft cannot be applied to a replacement."
				}),
				/* @__PURE__ */ X("button", {
					onClick: () => {
						t.drafts.delete(e), t.touch();
					},
					children: "Discard retired draft"
				})
			]
		}, e)),
		!c && /* @__PURE__ */ X("p", {
			class: "hint",
			children: e.enabled ? "Hardware controls are unavailable until the bus is ready." : "Enable this bus to use hardware controls."
		})
	] });
}
// src/groups.tsx
function Ge({ bus: e }) {
	let t = Z(), [n, r] = G(0), [i, a] = G(""), [o, s] = G("add"), [c, l] = G(null), [u, d] = G(null), [f, p] = G(""), m = e.groups?.find((e) => e.group_address === n), h = t.currentRows.filter((t) => t.kind === "output" && t.bus.bus === e.bus), g = t.currentRows.find((t) => t.kind === "group" && t.bus.bus === e.bus && t.item.group_address === n), _ = J(t.snapshot, e) && t.client.views.ready;
	function v() {
		let e = h.map((e) => ({
			key: e.key,
			generation: e.generation,
			name: e.item.name,
			before: m?.members?.includes(e.item.short_address) ?? !1,
			after: o === "add" && (m?.members?.includes(e.item.short_address) ?? !1) || Ee(i, e.item.name)
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
				if (t.progress = `Assigning group ${n.slot} · ${i.name}`, t.touch(), !Y(n.initial, t.snapshot, r, e.bus) || !t.currentRows.some((e) => e.key === n.groupKey)) throw Error("Group configuration changed after preview. Review the pattern again.");
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
				if (!Y(n, t.snapshot, n.inventory.revision, e.bus) || t.current(i.key).generation !== i.generation) throw Error("Configuration changed during verification. Start verification again.");
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
	return /* @__PURE__ */ X(y, { children: [/* @__PURE__ */ X(Re, {}), /* @__PURE__ */ X("div", {
		class: "groups-layout",
		children: [/* @__PURE__ */ X("aside", {
			class: "slots",
			children: [/* @__PURE__ */ X("h2", { children: "Groups" }), Array.from({ length: 16 }, (t, i) => {
				let a = e.groups?.find((e) => e.group_address === i);
				return /* @__PURE__ */ X("button", {
					class: n === i ? "selected" : "",
					onClick: () => {
						r(i), p("");
					},
					children: [/* @__PURE__ */ X("span", { children: i }), a?.name ?? "Unused slot"]
				});
			})]
		}), /* @__PURE__ */ X("section", { children: [
			/* @__PURE__ */ X("div", {
				class: "section-heading",
				children: [/* @__PURE__ */ X("h2", { children: ["Group ", n] }), /* @__PURE__ */ X("button", {
					disabled: !_ || !!t.busy,
					onClick: () => void t.run(x),
					children: "Verify all groups"
				})]
			}),
			m && g ? /* @__PURE__ */ X(y, { children: [
				/* @__PURE__ */ X("div", {
					class: "card group-title",
					children: [/* @__PURE__ */ X(Be, { row: g }), /* @__PURE__ */ X("span", {
						class: "hint",
						children: [
							m.members?.length ?? 0,
							" saved members · slot ",
							n
						]
					})]
				}),
				/* @__PURE__ */ X("div", {
					class: "toolbar pattern",
					children: [
						/* @__PURE__ */ X("input", {
							"aria-label": "Group name pattern",
							value: i,
							placeholder: "Name pattern · e.g. Kitchen*",
							onInput: (e) => a(e.currentTarget.value)
						}),
						/* @__PURE__ */ X("select", {
							"aria-label": "Pattern assignment mode",
							value: o,
							onChange: (e) => s(e.currentTarget.value),
							children: [/* @__PURE__ */ X("option", {
								value: "add",
								children: "Add matching fixtures"
							}), /* @__PURE__ */ X("option", {
								value: "replace",
								children: "Replace this group's members"
							})]
						}),
						/* @__PURE__ */ X("button", {
							disabled: !i || !_ || !!t.busy,
							onClick: v,
							children: "Preview pattern"
						})
					]
				}),
				/* @__PURE__ */ X("p", {
					class: "hint",
					children: "Checkboxes show saved membership. Verification compares all 16 hardware group bits."
				}),
				/* @__PURE__ */ X("div", {
					class: "device-table membership-table",
					children: h.map((e) => /* @__PURE__ */ X("label", {
						class: "membership-row",
						children: [
							/* @__PURE__ */ X("input", {
								type: "checkbox",
								"aria-label": `Assign ${e.item.name} to ${m.name}`,
								checked: m.members?.includes(e.item.short_address) ?? !1,
								disabled: !_ || !!t.busy,
								onChange: (r) => {
									let i = r.currentTarget.checked;
									t.run(() => t.group(e, n, i).then(() => {}));
								}
							}),
							/* @__PURE__ */ X("span", {
								class: "address",
								children: e.item.short_address
							}),
							/* @__PURE__ */ X("span", { children: e.item.name }),
							/* @__PURE__ */ X("button", {
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
			] }) : /* @__PURE__ */ X("div", {
				class: "card",
				children: [
					/* @__PURE__ */ X("h3", { children: ["Unused group slot ", n] }),
					/* @__PURE__ */ X("p", {
						class: "muted",
						children: "Save a group name before assigning fixtures. Existing hardware membership is adopted only after review."
					}),
					/* @__PURE__ */ X("div", {
						class: "toolbar",
						children: [/* @__PURE__ */ X("input", {
							"aria-label": "New group name",
							placeholder: `Group${n + 1}`,
							value: f,
							onInput: (e) => p(e.currentTarget.value)
						}), /* @__PURE__ */ X("button", {
							class: "primary",
							disabled: !!t.busy,
							onClick: () => void t.run(() => t.createGroup(e, n, f || `Group${n + 1}`)),
							children: "Create group"
						})]
					})
				]
			}),
			c && /* @__PURE__ */ X($, {
				title: "Review pattern assignment",
				close: () => l(null),
				children: [
					/* @__PURE__ */ X("p", { children: [
						c.changes.length,
						" changes in group ",
						c.slot,
						". Other groups stay unchanged."
					] }),
					/* @__PURE__ */ X("ul", { children: c.changes.map((e) => /* @__PURE__ */ X("li", { children: [
						e.after ? "Add" : "Remove",
						" ",
						e.name
					] }, e.key)) }),
					!c.changes.length && /* @__PURE__ */ X("p", { children: "No membership changes are needed." }),
					/* @__PURE__ */ X("div", {
						class: "dialog-actions",
						children: [/* @__PURE__ */ X("button", {
							onClick: () => l(null),
							children: "Cancel"
						}), /* @__PURE__ */ X("button", {
							class: "primary",
							disabled: !c.changes.length || !!t.busy,
							onClick: () => void t.run(b),
							children: "Apply reviewed changes"
						})]
					})
				]
			}),
			u && /* @__PURE__ */ X($, {
				title: "Verify group assignments",
				close: () => d(null),
				children: [/* @__PURE__ */ X("p", {
					class: "hint",
					children: "Compare every saved fixture's complete hardware mask, including unnamed group slots. Missing replies stay unresolved."
				}), u.map((e) => {
					let n = t.currentRows.find((t) => t.key === e.key), r = e.report, i = r.probe?.quality === "complete" && !r.probe?.stale, a = i && r.probe.mask !== r.expected_mask;
					return /* @__PURE__ */ X("section", {
						class: "verify-row",
						children: [
							/* @__PURE__ */ X("h3", { children: n?.item.name ?? "Removed fixture" }),
							/* @__PURE__ */ X("p", { children: i ? a ? `Different · saved ${r.expected_mask.toString(16).padStart(4, "0")} · hardware ${r.probe.mask.toString(16).padStart(4, "0")}` : "Assignments match" : "Unresolved hardware reply" }),
							a && n && /* @__PURE__ */ X("div", {
								class: "dialog-actions",
								children: [/* @__PURE__ */ X("button", {
									disabled: !!t.busy,
									onClick: () => void t.run(() => S(e, "adopt_groups")),
									children: "Adopt hardware assignments"
								}), /* @__PURE__ */ X("button", {
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
// src/app.tsx
function Ke({ number: e, bus: t, close: n }) {
	let r = Z(), [i, a] = G(t?.name ?? `Bus${e + 1}`), [o, s] = G(t?.enabled ?? !0);
	return /* @__PURE__ */ X($, {
		title: `Bus ${e + 1} settings`,
		close: n,
		children: /* @__PURE__ */ X("form", {
			onSubmit: (t) => {
				t.preventDefault(), r.run(async () => {
					await r.configureBus(e, i, o), n();
				});
			},
			children: [
				/* @__PURE__ */ X("label", {
					class: "field",
					children: ["Bus name", /* @__PURE__ */ X("input", {
						required: !0,
						"aria-label": "Bus name",
						value: i,
						onInput: (e) => a(e.currentTarget.value)
					})]
				}),
				/* @__PURE__ */ X("label", {
					class: "check-field",
					children: [/* @__PURE__ */ X("input", {
						type: "checkbox",
						checked: o,
						onChange: (e) => s(e.currentTarget.checked)
					}), "Enable bus"]
				}),
				/* @__PURE__ */ X("p", {
					class: "hint",
					children: "Saved devices keep their identity when the bus is renamed or disabled."
				}),
				/* @__PURE__ */ X("div", {
					class: "dialog-actions",
					children: [/* @__PURE__ */ X("button", {
						type: "button",
						onClick: n,
						children: "Cancel"
					}), /* @__PURE__ */ X("button", {
						class: "primary",
						disabled: !!r.busy || !r.client.views.ready,
						children: "Save bus"
					})]
				})
			]
		})
	});
}
function qe() {
	let e = Z(), [t, n] = G(null), r = e.snapshot;
	return /* @__PURE__ */ X(y, { children: [
		/* @__PURE__ */ X("header", {
			class: "page-heading",
			children: /* @__PURE__ */ X("div", { children: [
				/* @__PURE__ */ X("p", {
					class: "eyebrow",
					children: "CONTROLLER"
				}),
				/* @__PURE__ */ X("h1", { children: r.identity.controller }),
				/* @__PURE__ */ X("p", {
					class: "muted",
					children: "DALI bus management"
				})
			] })
		}),
		/* @__PURE__ */ X("div", {
			class: "summary-grid",
			children: [
				/* @__PURE__ */ X("div", {
					class: "card",
					children: [/* @__PURE__ */ X("span", {
						class: "muted",
						children: "Controller"
					}), /* @__PURE__ */ X("h2", { children: r.phase })]
				}),
				/* @__PURE__ */ X("div", {
					class: "card",
					children: [/* @__PURE__ */ X("span", {
						class: "muted",
						children: "UART / HAT"
					}), /* @__PURE__ */ X("h2", { children: r.hardware.state })]
				}),
				/* @__PURE__ */ X("div", {
					class: "card",
					children: [/* @__PURE__ */ X("span", {
						class: "muted",
						children: "Inventory revision"
					}), /* @__PURE__ */ X("h2", { children: r.inventory.revision })]
				})
			]
		}),
		/* @__PURE__ */ X("div", {
			class: "section-heading",
			children: [/* @__PURE__ */ X("h2", { children: "Buses" }), /* @__PURE__ */ X("span", {
				class: "hint",
				children: "Select a bus to discover and identify its devices."
			})]
		}),
		/* @__PURE__ */ X("div", {
			class: "bus-grid",
			children: Array.from({ length: 4 }, (e, t) => {
				let i = r.inventory.buses.find((e) => e.bus === t);
				return /* @__PURE__ */ X("article", {
					class: "card bus-card",
					children: [
						/* @__PURE__ */ X("div", {
							class: "section-heading",
							children: [/* @__PURE__ */ X("h2", { children: i?.name ?? `Bus${t + 1}` }), /* @__PURE__ */ X("span", {
								class: `pill ${i && J(r, i) ? "" : "unknown"}`,
								children: i ? i.enabled ? "Enabled" : "Disabled" : "Not configured"
							})]
						}),
						/* @__PURE__ */ X("p", {
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
						/* @__PURE__ */ X("div", {
							class: "toolbar",
							children: [i && /* @__PURE__ */ X(Q, {
								class: "button primary",
								href: `/buses/${t}`,
								children: "Open bus"
							}), /* @__PURE__ */ X("button", {
								onClick: () => n(t),
								children: i ? "Bus settings" : "Set up bus"
							})]
						})
					]
				}, t);
			})
		}),
		t !== null && /* @__PURE__ */ X(Ke, {
			number: t,
			bus: r.inventory.buses.find((e) => e.bus === t),
			close: () => n(null)
		})
	] });
}
function Je({ number: e, tab: t }) {
	let n = Z(), r = n.snapshot.inventory.buses.find((t) => t.bus === e), [i, a] = G(!1);
	return /* @__PURE__ */ X(y, { children: [
		/* @__PURE__ */ X("header", {
			class: "page-heading",
			children: [/* @__PURE__ */ X("div", { children: [
				/* @__PURE__ */ X("p", {
					class: "eyebrow",
					children: "BUS MANAGEMENT"
				}),
				/* @__PURE__ */ X("h1", { children: [
					n.snapshot.identity.controller,
					"/",
					r?.name ?? `Bus${e + 1}`
				] }),
				/* @__PURE__ */ X("p", {
					class: "muted",
					children: "Discover, identify and name the devices on this bus."
				})
			] }), /* @__PURE__ */ X("button", {
				onClick: () => a(!0),
				children: "Bus settings"
			})]
		}),
		r ? /* @__PURE__ */ X(y, { children: [/* @__PURE__ */ X("nav", {
			class: "tabs",
			"aria-label": "Bus pages",
			children: [
				["", "Devices"],
				["/groups", "Groups"],
				["/scenes", "Scenes"]
			].map(([n, r]) => /* @__PURE__ */ X(Q, {
				class: t === n ? "selected" : "",
				href: `/buses/${e}${n}`,
				children: r
			}))
		}), t === "/groups" ? /* @__PURE__ */ X(Ge, { bus: r }, `groups-${e}`) : t === "/scenes" ? /* @__PURE__ */ X("div", {
			class: "card",
			children: [/* @__PURE__ */ X("h2", { children: "Native DALI scenes" }), /* @__PURE__ */ X("p", {
				class: "muted",
				children: "Scene commissioning and recall are not available in this development build."
			})]
		}) : /* @__PURE__ */ X(We, { bus: r }, `devices-${e}`)] }) : /* @__PURE__ */ X("div", {
			class: "card",
			children: [
				/* @__PURE__ */ X("h2", { children: "Set up this bus" }),
				/* @__PURE__ */ X("p", {
					class: "muted",
					children: "Save a bus name and enable it before starting discovery."
				}),
				/* @__PURE__ */ X("button", {
					class: "primary",
					onClick: () => a(!0),
					children: "Set up bus"
				})
			]
		}),
		i && /* @__PURE__ */ X(Ke, {
			number: e,
			bus: r,
			close: () => a(!1)
		})
	] });
}
function Ye() {
	let e = Z().snapshot;
	function t() {
		let t = URL.createObjectURL(new Blob([JSON.stringify(e.inventory, null, 2)], { type: "application/json" })), n = document.createElement("a");
		n.href = t, n.download = "dali-inventory-report.json", n.click(), setTimeout(() => URL.revokeObjectURL(t), 0);
	}
	return /* @__PURE__ */ X(y, { children: [
		/* @__PURE__ */ X("header", {
			class: "page-heading",
			children: /* @__PURE__ */ X("div", { children: [/* @__PURE__ */ X("p", {
				class: "eyebrow",
				children: "CONTROLLER"
			}), /* @__PURE__ */ X("h1", { children: "Settings" })] })
		}),
		/* @__PURE__ */ X("section", {
			class: "card settings-section",
			children: [
				/* @__PURE__ */ X("h2", { children: e.identity.controller }),
				/* @__PURE__ */ X("dl", { children: [
					/* @__PURE__ */ X("dt", { children: "Version" }),
					/* @__PURE__ */ X("dd", { children: e.identity.version }),
					/* @__PURE__ */ X("dt", { children: "Build UTC" }),
					/* @__PURE__ */ X("dd", { children: e.identity.build_utc }),
					/* @__PURE__ */ X("dt", { children: "Inventory epoch" }),
					/* @__PURE__ */ X("dd", {
						class: "mono",
						children: e.inventory.inventory_epoch
					}),
					/* @__PURE__ */ X("dt", { children: "Revision" }),
					/* @__PURE__ */ X("dd", { children: e.inventory.revision })
				] }),
				/* @__PURE__ */ X("p", {
					class: "hint",
					children: "Listener, MQTT, UART and password options are supplied when the bridge starts. Bus settings are available on each bus page."
				})
			]
		}),
		/* @__PURE__ */ X("section", {
			class: "card settings-section",
			children: [
				/* @__PURE__ */ X("h2", { children: "Inventory report" }),
				/* @__PURE__ */ X("p", {
					class: "muted",
					children: "Download the current published inventory and identity history. This report is not a restorable configuration backup."
				}),
				/* @__PURE__ */ X("button", {
					onClick: t,
					children: "Download inventory report"
				})
			]
		})
	] });
}
function Xe() {
	let e = Z(), t = e.snapshot;
	return /* @__PURE__ */ X(y, { children: [
		/* @__PURE__ */ X("header", {
			class: "page-heading",
			children: /* @__PURE__ */ X("div", { children: [/* @__PURE__ */ X("p", {
				class: "eyebrow",
				children: "CONTROLLER"
			}), /* @__PURE__ */ X("h1", { children: "Diagnostics" })] })
		}),
		/* @__PURE__ */ X("div", {
			class: "summary-grid",
			children: [
				/* @__PURE__ */ X("section", {
					class: "card",
					children: [
						/* @__PURE__ */ X("h2", { children: "UART / HAT" }),
						/* @__PURE__ */ X("p", { children: t.hardware.state }),
						t.hardware.buses?.map((e) => /* @__PURE__ */ X("p", {
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
				/* @__PURE__ */ X("section", {
					class: "card",
					children: [
						/* @__PURE__ */ X("h2", { children: "Browser connection" }),
						/* @__PURE__ */ X("p", { children: e.client.views.ready ? "Live" : "Stale · reconnecting" }),
						/* @__PURE__ */ X("p", {
							class: "hint",
							children: [e.client.lost, " input actions reported lost"]
						})
					]
				}),
				/* @__PURE__ */ X("section", {
					class: "card",
					children: [/* @__PURE__ */ X("h2", { children: "MQTT" }), /* @__PURE__ */ X("p", {
						class: "muted",
						children: "Connection status is not exposed by this build."
					})]
				})
			]
		}),
		/* @__PURE__ */ X("section", {
			class: "card settings-section",
			children: [/* @__PURE__ */ X("h2", { children: "Recent commands" }), e.client.jobs.length ? /* @__PURE__ */ X("div", {
				class: "job-list",
				children: e.client.jobs.map((e) => /* @__PURE__ */ X("article", { children: [
					/* @__PURE__ */ X("strong", { children: e.command }),
					/* @__PURE__ */ X("span", {
						class: "pill unknown",
						children: e.status
					}),
					/* @__PURE__ */ X("p", {
						class: "hint",
						children: [
							e.target ?? "Controller",
							" · ",
							e.id
						]
					}),
					e.error && /* @__PURE__ */ X("p", {
						class: "error",
						children: e.error.message ?? e.error.code
					})
				] }, e.id))
			}) : /* @__PURE__ */ X("p", {
				class: "muted",
				children: "No commands recorded in this bridge session."
			})]
		}),
		/* @__PURE__ */ X("section", {
			class: "card settings-section",
			children: [/* @__PURE__ */ X("h2", { children: "Recent input actions" }), e.client.events.length ? /* @__PURE__ */ X("div", {
				class: "job-list",
				children: e.client.events.map((e) => /* @__PURE__ */ X("article", { children: [/* @__PURE__ */ X("strong", { children: e.target }), /* @__PURE__ */ X("pre", { children: JSON.stringify(e, null, 2) })] }))
			}) : /* @__PURE__ */ X("p", {
				class: "muted",
				children: "Operate an input after discovery to see its actions here."
			})]
		})
	] });
}
function Ze() {
	let [e] = G(() => new Fe()), [, t] = G(0), [n, r] = G(location.pathname), [i, a] = G(!1);
	pe(() => {
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
	return /* @__PURE__ */ X(Le.Provider, {
		value: e,
		children: /* @__PURE__ */ X("div", {
			class: "layout",
			children: [/* @__PURE__ */ X("aside", {
				class: `sidebar ${i ? "open" : ""}`,
				children: [
					/* @__PURE__ */ X("div", {
						class: "brand",
						children: [
							/* @__PURE__ */ X("span", {
								class: "brand-icon",
								children: "D"
							}),
							/* @__PURE__ */ X("div", { children: [/* @__PURE__ */ X("strong", { children: "DALI Bridge" }), /* @__PURE__ */ X("span", { children: o?.identity.controller ?? "Connecting…" })] }),
							/* @__PURE__ */ X("button", {
								class: "nav-close",
								"aria-label": "Close navigation",
								onClick: () => a(!1),
								children: "×"
							})
						]
					}),
					/* @__PURE__ */ X("p", {
						class: "nav-label",
						children: "CONTROLLER"
					}),
					/* @__PURE__ */ X(Q, {
						class: n === "/" ? "selected" : "",
						href: "/",
						children: "Overview"
					}),
					/* @__PURE__ */ X("p", {
						class: "nav-label",
						children: "DALI BUSES"
					}),
					o?.inventory.buses.map((e) => /* @__PURE__ */ X(Q, {
						class: s && Number(s[1]) === e.bus ? "selected" : "",
						href: `/buses/${e.bus}`,
						children: [/* @__PURE__ */ X("span", { children: e.name }), /* @__PURE__ */ X("span", {
							class: "bus-count",
							children: e.outputs.length
						})]
					})),
					/* @__PURE__ */ X("div", {
						class: "sidebar-bottom",
						children: [
							/* @__PURE__ */ X(Q, {
								class: n === "/settings" ? "selected" : "",
								href: "/settings",
								children: "Settings"
							}),
							/* @__PURE__ */ X(Q, {
								class: n === "/diagnostics" ? "selected" : "",
								href: "/diagnostics",
								children: "Diagnostics"
							}),
							/* @__PURE__ */ X("p", {
								class: "hint",
								children: [
									o?.identity.version ?? "DALI v2",
									/* @__PURE__ */ X("br", {}),
									o?.identity.build_utc
								]
							})
						]
					})
				]
			}), /* @__PURE__ */ X("div", {
				class: "main-shell",
				children: [
					/* @__PURE__ */ X("header", {
						class: "topbar",
						children: [
							/* @__PURE__ */ X("button", {
								class: "nav-toggle",
								"aria-label": "Open navigation",
								onClick: () => a(!0),
								children: "☰"
							}),
							/* @__PURE__ */ X("span", {
								class: "muted",
								children: o?.identity.controller ?? "DALI controller"
							}),
							/* @__PURE__ */ X("span", { class: "spacer" }),
							/* @__PURE__ */ X("span", {
								class: `pill ${e.client.views.ready ? "" : "unknown"}`,
								children: e.client.views.ready ? "Live" : "Connecting"
							}),
							/* @__PURE__ */ X("button", {
								disabled: e.signingOut,
								onClick: () => void e.logout(),
								children: e.signingOut ? "Signing out…" : "Sign out"
							})
						]
					}),
					/* @__PURE__ */ X("main", { children: [
						(e.error || e.client.error) && /* @__PURE__ */ X("div", {
							class: "banner error",
							role: "alert",
							children: e.error || e.client.error
						}),
						e.notice && /* @__PURE__ */ X("div", {
							class: "banner notice",
							role: "status",
							children: e.notice
						}),
						o ? s ? /* @__PURE__ */ X(Je, {
							number: Number(s[1]),
							tab: s[2] ?? ""
						}, s[1]) : n === "/settings" ? /* @__PURE__ */ X(Ye, {}) : n === "/diagnostics" ? /* @__PURE__ */ X(Xe, {}) : n === "/" ? /* @__PURE__ */ X(qe, {}) : /* @__PURE__ */ X("div", {
							class: "card",
							children: [/* @__PURE__ */ X("h1", { children: "Page not found" }), /* @__PURE__ */ X(Q, {
								href: "/",
								children: "Return to overview"
							})]
						}) : /* @__PURE__ */ X("div", {
							class: "card",
							children: [/* @__PURE__ */ X("h1", { children: "Connecting to the controller…" }), /* @__PURE__ */ X("p", {
								class: "muted",
								children: "Waiting for the live inventory."
							})]
						})
					] }),
					/* @__PURE__ */ X("footer", { children: [
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
F(/* @__PURE__ */ X(Ze, {}), document.getElementById("app"));
