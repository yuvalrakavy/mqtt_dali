// node_modules/preact/dist/preact.mjs
var e, t, n, r, i, a, o, s, c, l, u = {}, d = [], f = /^m(i|n|o|s|text|space)$/, p = Array.isArray, m = d.slice, h = Object.assign;
function g(e) {
	e && e.parentNode && e.remove();
}
function _(n, r, i, a, o) {
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
function v(e) {
	return e.children;
}
function y(e, t) {
	this.props = e, this.context = t, this.__g = 0;
}
function b(e, t) {
	if (t == null) return e.__ ? b(e.__, e.__i + 1) : null;
	for (var n; t < e.__k.length; t++) if ((n = e.__k[t]) && n.__e) return n.__e;
	return typeof e.type != "function" || e.props.__P ? null : b(e);
}
function x(e) {
	if ((e = e.__) && e.__c && !e.props.__P) return e.__e = null, e.__k.some(function(t) {
		return t && (e.__e = t.__e);
	}), x(e);
}
function S(t) {
	(8 & t.__g || !(t.__g |= 8) || !n.push(t) || i++) && r == e.debounceRendering || ((r = e.debounceRendering) || queueMicrotask)(C);
}
function C() {
	var t, r, o, s, c, l, u, d, f;
	try {
		for (r = 1; n.length;) n.length > r && n.sort(a), t = n.shift(), r = n.length, 8 & t.__g && (s = void 0, c = void 0, l = (c = (o = t).__v).__e, u = [], d = [], (f = o.__P) && ((s = h({ constructor: void 0 }, c)).__v = c.__v + 1, e.vnode && e.vnode(s), j(f, s, c, o.__n, f.namespaceURI, 32 & c.__u ? [l] : null, u, l || b(c), 32 & c.__u, d), s.__v = c.__v, s.__.__k[s.__i] = s, M(u, s, d), c.__ = c.__e = null, s.__e != l && x(s)));
	} finally {
		n.length = i = 0;
	}
}
function w(e, t, n, r, i, a, o, s, c, l, f) {
	var p, m, h, g, _, v, y = r.__k || d, b = t.length;
	for (c = T(n, t, y, c, b), p = 0; p < b; p++) (h = n.__k[p]) != null && (m = ~h.__i && y[h.__i] || u, h.__i = p, v = j(e, h, m, i, a, o, s, c, l, f), g = h.__e, (m.ref != h.ref || 8 & m.__u) && (m.ref && N(m.ref, null, h, m), h.ref && f.push(h.ref, h.__c || g, h)), _ = _ || g, 4 & h.__u ? (c = E(h, c, e, !m.__v), m.__e && (m.__e = null)) : typeof h.type == "function" && v !== void 0 ? c = v : g && (c = g.nextSibling), h.__u &= -7);
	return n.__e = _, c;
}
function T(e, t, n, r, i) {
	var a, o, s, c, l, u, d, f, m, h, g = n.length, y = g, x = 0, S = !1, C = e.__k = Array(i);
	for (a = 0; a < i; a++) (o = t[a]) != null && typeof o != "boolean" && typeof o != "function" ? (typeof o != "object" || o.constructor == String ? o = C[a] = _(null, o) : p(o) ? o = C[a] = _(v, { children: o }) : o.constructor === void 0 && o.__b ? o = C[a] = _(o.type, o.props, o.key, o.ref, o.__v) : C[a] = o, c = a + x, o.__ = e, o.__b = e.__b + 1, s = null, ~(l = o.__i = D(o, n, c, y)) && (y--, (s = n[l]) && (s.__u |= 2)), s && s.__v ? (o.__u |= 2, l == c - 1 ? x-- : l == c + 1 ? x++ : l != c && (l > c ? x-- : x++, S = !0)) : (~l || (i > g ? x-- : i < g && x++), typeof o.type != "function" && (o.__u |= 4))) : C[a] = null;
	if (S) {
		for (u = [], d = [], a = 0; a < i; a++) if ((o = C[a]) && 2 & o.__u) {
			for (f = 0, m = u.length; f < m;) u[h = f + m >> 1] < o.__i ? f = h + 1 : m = h;
			u[f] = o.__i, d[a] = f + 1;
		}
		for (x = u.length; a--;) d[a] && (d[a] == x ? x-- : C[a].__u |= 4);
	}
	if (y) for (a = 0; a < g; a++) !(s = n[a]) || 2 & s.__u || (s.__e == r && (r = b(s)), ne(s, s));
	return r;
}
function E(e, t, n, r) {
	var i, a, o;
	if (typeof e.type == "function") {
		if (e.props.__P) return t;
		if (i = e.__k) for (a = 0; a < i.length; a++) i[a] && (i[a].__ = e, t = E(i[a], t, n, !1));
		return t;
	}
	for (t && !t.parentNode && (t = b(e)) && !t.parentNode && (t = null), o = t; o && o.nodeType == 8;) o = o.nextSibling;
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
	var f, m, _, x, S, C, T, E, D, O, k, A, j, M, N, P, F, I, L, R, z = n.type;
	if (n.constructor !== void 0) return null;
	if (128 & r.__u && (l = 32 & r.__u, f = r.__c.__z)) {
		if (n.__u |= l, m = o = [], f.nodeType == 8) for (_ = 1, x = f.nextSibling; x; x = x.nextSibling) {
			if (x.nodeType == 8) {
				if (x.data.startsWith("$s")) _++;
				else if (x.data.startsWith("/$s") && !--_) break;
			}
			o.push(x);
		}
		else o.push(f);
		c = o[0];
	}
	(f = e.__b) && f(n);
	n: if (typeof z == "function") {
		S = s.length;
		try {
			if (O = n.props, k = (f = z.prototype) && f.render, A = (f = z.contextType) && i[f.__c], j = f ? A ? A.props.value : f.__ : i, r.__c ? 2 & (C = n.__c = r.__c).__g && (C.__g |= 1) : (k ? n.__c = C = new z(O, j) : (n.__c = C = new y(O, j), C.constructor = z, C.render = re), A && A.sub(C), C.state || (C.state = {}), C.__n = i, C.__g |= 8, C.__h = [], C.__k = []), k && (C.__s || (C.__s = C.state), z.getDerivedStateFromProps && (C.__s == C.state && (C.__s = h({}, C.__s)), h(C.__s, z.getDerivedStateFromProps(O, C.__s)))), T = C.props, E = C.state, C.__v = n, r.__c) {
				if (k && !z.getDerivedStateFromProps && O !== T && C.componentWillReceiveProps && C.componentWillReceiveProps(O, j), n.__v == r.__v && !(8 & C.__g) || !(4 & C.__g) && C.shouldComponentUpdate && !1 === C.shouldComponentUpdate(O, C.__s, j)) {
					n.__v != r.__v && (C.props = O, C.state = C.__s, C.__g &= -9), n.__e = r.__e, n.__k = r.__k, n.__k.some(function(e) {
						e && (e.__ = n);
					}), d.push.apply(C.__h, C.__k), C.__k = [], C.__h.length && s.push(C), c = b(r);
					break n;
				}
				C.componentWillUpdate && C.componentWillUpdate(O, C.__s, j), k && C.componentDidUpdate && C.__h.push(function() {
					C.componentDidUpdate(T, E, D);
				});
			} else k && !z.getDerivedStateFromProps && C.componentWillMount && C.componentWillMount(), k && C.componentDidMount && C.__h.push(C.componentDidMount);
			if (C.context = j, C.props = O, C.__P = t, C.__g &= -5, M = e.__r, N = 0, k) C.state = C.__s, C.__g &= -9, M && M(n), f = C.render(C.props, C.state, C.context), d.push.apply(C.__h, C.__k), C.__k = [];
			else do
				C.__g &= -9, M && M(n), f = C.render(C.props, C.state, C.context), C.state = C.__s;
			while (8 & C.__g && ++N < 25);
			C.state = C.__s, C.getChildContext && (i = h({}, i, C.getChildContext())), k && r.__c && C.getSnapshotBeforeUpdate && (D = C.getSnapshotBeforeUpdate(T, E)), P = f && f.type === v && f.key == null ? f.props.children : f, O.__P && (f = c, a = (t = O.__P).namespaceURI, l = o = null, r.props && r.props.__P != t && (r.__k.some(function(e) {
				e && ne(e, e);
			}), r.__k = null), c = r.__k ? b(r, 0) : null), c = w(t, p(P) ? P : [P], n, r, i, a, o, s, c, l, u), O.__P && (n.__e = null, c = f), n.__u &= -161, 128 & r.__u && (C.__z = null), m && m.some(g), C.__h.length && s.push(C), 1 & C.__g && (C.__g &= -4);
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
				} else o && o.some(g);
			} else n.__e = r.__e;
			n.__k || (n.__k = r.__k || []), t.then || ee(n), e.__e(t, n, r);
		}
	} else c = n.__e = te(r.__e, n, r, i, a, o, s, l, u, t);
	return (f = e.diffed) && f(n), 128 & n.__u ? void 0 : c;
}
function ee(e) {
	e && (e.__c && (e.__c.__g |= 4), e.__k && e.__k.some(ee));
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
function te(t, n, r, i, a, o, s, c, l, d) {
	var h, _, v, y, x, S, C, T, E, D = r.props || u, O = n.props, A = n.type;
	if (A == "svg" ? a = "http://www.w3.org/2000/svg" : A == "math" ? a = "http://www.w3.org/1998/Math/MathML" : a || (a = "http://www.w3.org/1999/xhtml"), o) {
		for (h = 0; h < o.length; h++) if ((x = o[h]) && (A ? x.localName == A : x.nodeType == 3)) {
			t = x, o[h] = null;
			break;
		}
	}
	if (!t) {
		if (T = d.ownerDocument || document, !A) return T.createTextNode(O);
		t = T.createElementNS(a, A, O.is && O), c && (e.__m && e.__m(n, o), c = !1), o = null;
	}
	if (A) {
		if (d = A == "template" ? t.content : t, o = A == "textarea" && O.defaultValue != null ? null : o && m.call(d.childNodes), !c && o) for (D = {}, h = 0; h < t.attributes.length; h++) D[(x = t.attributes[h]).name] = x.value;
		for (h in D) x = D[h], h == "dangerouslySetInnerHTML" ? v = x : h == "children" || h in O || h == "value" && "defaultValue" in O || h == "checked" && "defaultChecked" in O || k(t, h, null, x, a);
		for (h in E = 1 & r.__u, O) x = O[h], h == "children" ? y = x : h == "dangerouslySetInnerHTML" ? _ = x : h == "value" ? S = x : h == "checked" ? C = x : c && typeof x != "function" || !(D[h] !== x || E && x != null) || k(t, h, x, D[h], a);
		_ ? (c || v && (_.__html == v.__html || _.__html == t.innerHTML) || (t.innerHTML = _.__html), n.__k = []) : (v && (t.textContent = ""), (A == "foreignObject" || a == "http://www.w3.org/1998/Math/MathML" && f.test(A)) && (a = "http://www.w3.org/1999/xhtml"), w(d, p(y) ? y : [y], n, r, i, a, o, s, o ? o[0] : r.__k && b(r, 0), c, l), o && o.some(g)), c && A != "textarea" || (h = "value", A == "progress" && S == null ? t.removeAttribute(h) : S == null || S === t[h] && (A != "progress" || S) || k(t, h, S, D[h], a), h = "checked", C != null && C != t[h] && k(t, h, C, D[h], a));
	} else D === O || c && t.data == O || (t.data = O);
	return t;
}
function N(t, n, r, i) {
	try {
		typeof t == "function" ? n ? n.__x = t(n) || 1 : i && (i = i.__c || i.__e) && (n = i.__x) && (i.__x = null, typeof n == "function" ? n() : t(null)) : t.current = n;
	} catch (t) {
		e.__e(t, r);
	}
}
function ne(t, n, r) {
	var i, a;
	if (e.unmount && e.unmount(t), !(i = t.ref) || i.current && i.current != t.__e || N(i, null, n, t), i = t.__c) {
		if (i.componentWillUnmount) try {
			i.componentWillUnmount();
		} catch (t) {
			e.__e(t, n);
		}
		i.__P = i.__n = null;
	}
	if (i = t.__k) for (a = 0; a < i.length; a++) i[a] && ne(i[a], n, typeof t.type != "function" || r && !t.props.__P);
	(i = t.__e) && (r || g(i), i.__e && (i.__e = null)), t.__e = t.__c = t.__ = null;
}
function re(e, t, n) {
	return this.constructor(e, n);
}
function P(t, n) {
	var r, i, a, o;
	e.__ && e.__(t, n), n.nodeType == 9 && (n = n.documentElement), i = (r = t && 32 & t.__u) ? null : n.__k, n.__k = _(v, { children: [t] }), a = [], o = [], j(n, n.__k, i || u, u, n.namespaceURI, i ? null : n.firstChild ? m.call(n.childNodes) : null, a, i ? i.__e : n.firstChild, r, o), M(a, n.__k, o), n.__k.props.children = null;
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
} }, t = 0, y.prototype.setState = function(e, t) {
	var n = this.__s;
	n && n != this.state || (n = this.__s = h({}, this.state)), typeof e == "function" && (e = e(h({}, n), this.props)), e && (h(n, e), this.__v && (t && this.__k.push(t), S(this)));
}, y.prototype.forceUpdate = function(e) {
	this.__v && (this.__g |= 4, e && this.__h.push(e), S(this));
}, y.prototype.render = v, n = [], i = 0, a = function(e, t) {
	return e.__v.__b - t.__v.__b;
}, o = Symbol(), s = /(PointerCapture)$|Capture$/i, c = 0, l = {};
// node_modules/preact/hooks/dist/hooks.mjs
var F, I, L, R, z = Object.is, B = 0, ie = [], V = [], H = e, ae = H.__b, oe = H.__r, se = H.diffed, ce = H.__c, le = H.unmount, ue = H.__;
function U(e, t) {
	H.__h && H.__h(I, e, B || t), B = 0;
	var n = I.__H || (I.__H = {
		__: [],
		__h: []
	});
	return e >= n.__.length && n.__.push({}), n.__[e];
}
function W(e) {
	return B = 1, de(ye, e);
}
function de(e, t, n) {
	var r = U(F++, 2);
	if (r.t = e, !r.__c && (r.__ = [n ? n(t) : ye(void 0, t), function(e) {
		var t = r.__N ? r.__N[0] : r.__[0], n = r.t(t, e);
		z(t, n) || (r.__N = [n, r.__[1]], r.__c.setState({}));
	}], r.__c = I, !I.__f)) {
		I.__f = !0;
		var i = I.shouldComponentUpdate;
		I.shouldComponentUpdate = function(e, t, n) {
			var r = this.__H;
			if (!r) return !0;
			var a = !1, o = this.props != e;
			if (r.__.some(function(e) {
				e.__N && (a = !0, z(e.__[0], e.__N[0]) || (o = !0));
			}), i) {
				var s = i.call(this, e, t, n);
				return a ? s || o : s;
			}
			return !a || o;
		};
	}
	return r.__;
}
function G(e, t) {
	var n = U(F++, 3);
	!H.__s && ve(n.__H, t) && (n.__P = !0, n.__ = e, n.u = t, I.__H.__h.push(n));
}
function fe(e) {
	return B = 5, pe(function() {
		return { current: e };
	}, []);
}
function pe(e, t) {
	var n = U(F++, 7);
	return ve(n.__H, t) && (n.__ = e(), n.__H = t), n.__;
}
function me() {
	var e;
	do {
		for (; e = V.shift();) try {
			K(e);
		} catch (t) {
			H.__e(t, { __: (e = e.__P) && e.__v });
		}
		for (; e = ie.shift();) {
			var t = e.__H;
			if (e.__P && t) try {
				t.__h.some(K), t.__h.some(q), t.__h = [];
			} catch (n) {
				t.__h = [], H.__e(n, e.__v);
			}
		}
	} while (V.length);
}
H.__b = function(e) {
	I = null, ae && ae(e);
}, H.__ = function(e, t) {
	e && t.__k && t.__k.__m && (e.__m = t.__k.__m), ue && ue(e, t);
}, H.__r = function(e) {
	oe && oe(e), F = 0;
	var t = (I = e.__c).__H;
	t && (L == I ? I.__h = [] : (t.__h.some(K), t.__h.some(q), F = 0), t.__h = [], t.__.some(function(e) {
		e.__N && (e.__ = e.__N), e.u = e.__N = void 0;
	})), L = I;
}, H.diffed = function(e) {
	se && se(e);
	var t = e.__c;
	t && t.__H && (t.__H.__h.length && _e(ie.push(t)), t.__H.__.some(function(e) {
		e.u && (e.__H = e.u);
	})), L = I = null;
}, H.__c = function(e, t) {
	t.some(function(e) {
		try {
			e.__h.some(K), e.__h = e.__h.filter(function(e) {
				return !e.__ || q(e);
			});
		} catch (n) {
			t.some(function(e) {
				e.__h && (e.__h = []);
			}), t = [], H.__e(n, e.__v);
		}
	}), ce && ce(e, t);
}, H.unmount = function(e) {
	le && le(e);
	var t, n, r = e.__c;
	r && r.__H && (r.__H.__.some(function(r) {
		try {
			if (r.__P && r.__c) {
				if (n === void 0) {
					for (n = e.__; n && (!n.__c || !n.__c.__P);) n = n.__;
					n = n && n.__c;
				}
				r.__P = n, _e(V.push(r));
			} else K(r);
		} catch (e) {
			t = e;
		}
	}), r.__H = void 0, t && H.__e(t, r.__v));
};
var he = typeof requestAnimationFrame == "function";
function ge(e) {
	var t, n = function() {
		clearTimeout(r), he && cancelAnimationFrame(t), setTimeout(e);
	}, r = setTimeout(n, 35);
	he && (t = requestAnimationFrame(n));
}
function _e(e) {
	e != 1 && R == H.requestAnimationFrame || ((R = H.requestAnimationFrame) || ge)(me);
}
function K(e) {
	var t = I, n = e.__c;
	typeof n == "function" && (e.__c = void 0, n()), I = t;
}
function q(e) {
	var t = I;
	e.__c = e.__(), I = t;
}
function ve(e, t) {
	return !e || e.length != t.length || t.some(function(t, n) {
		return !z(t, e[n]);
	});
}
function ye(e, t) {
	return typeof t == "function" ? t(e) : t;
}
// node_modules/preact/jsx-runtime/dist/jsxRuntime.mjs
var be = 0;
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
		__v: --be,
		__i: -1,
		__u: 0
	};
	return (a || o) && (u.__source = a, u.__self = o), e.vnode && e.vnode(u), u;
}
// src/app.tsx
var Y = "";
async function X(e, t) {
	let n = new AbortController(), r = setTimeout(() => n.abort(), 3e3);
	try {
		let r = await fetch(`/api/${e}`, {
			method: t ? "POST" : "GET",
			headers: t ? {
				"Content-Type": "application/json",
				"X-Emulator-CSRF": Y
			} : {},
			body: t ? JSON.stringify(t) : void 0,
			signal: n.signal
		}), i = await r.json();
		if (!r.ok) throw r.status === 403 && (Y = ""), Error(i.error || `Request failed (${r.status})`);
		return i;
	} finally {
		clearTimeout(r);
	}
}
function xe(e, t) {
	if (!e || typeof e != "object" || typeof e.id != "string" || !e.id) throw Error("Fixture ID is required");
	if (t) {
		if (!Array.isArray(e.instances) || e.instances.length > 32 || e.instances.some((e) => !Number.isInteger(e) || e < 0 || e > 31)) throw Error("Input instances must be an array of native types 0–31");
		for (let t of [
			"values",
			"feedback",
			"initial_settings",
			"initial_physical"
		]) if (e[t] !== void 0 && !Array.isArray(e[t])) throw Error(`${t} must be an array`);
		for (let t of e.values ?? []) if (!t || typeof t != "object" || !Number.isInteger(t.instance) || t.instance < 0 || t.instance >= e.instances.length || !Number.isInteger(t.resolution) || t.resolution < 1 || t.resolution > 255 || !Array.isArray(t.bytes) || t.bytes.some((e) => !Number.isInteger(e) || e < 0 || e > 255)) throw Error("Invalid queried value profile");
		for (let t of e.feedback ?? []) if (!t || typeof t != "object" || t.instance !== null && (!Number.isInteger(t.instance) || t.instance < 0 || t.instance >= e.instances.length)) throw Error("Invalid feedback profile");
	} else if (e.scenes !== void 0 && (!Array.isArray(e.scenes) || e.scenes.length !== 16 || e.scenes.some((e) => !Number.isInteger(e) || e < 0 || e > 255))) throw Error("Scenes require sixteen native levels 0–255");
}
function Se(e) {
	if (e?.version !== 1 || !Array.isArray(e.buses) || !e.buses.length || e.buses.length > 4) throw Error("Expected a version 1 scenario with 1–4 buses");
	for (let t of e.buses) {
		if (!t || !Number.isInteger(t.number) || t.number < 0 || t.number > 3) throw Error("Invalid bus number");
		for (let e of ["outputs", "inputs"]) {
			if (t[e] !== void 0 && !Array.isArray(t[e])) throw Error("Fixture list must be an array");
			for (let n of t[e] ?? []) xe(n, e === "inputs");
		}
	}
}
var Z = (e) => structuredClone(e);
function Ce(e, t) {
	let n = URL.createObjectURL(new Blob([JSON.stringify(e, null, 2) + "\n"], { type: "application/json" })), r = document.createElement("a");
	r.href = n, r.download = t, r.click(), setTimeout(() => URL.revokeObjectURL(n), 1e3);
}
var Q = {
	1: "Button",
	2: "Absolute input",
	3: "Occupancy",
	4: "Light sensor"
};
function we(e) {
	let t = `${e}-${crypto.randomUUID().slice(0, 8)}`;
	return e === "dimmer" || e === "relay" || e === "colour" ? {
		id: t,
		short_address: null,
		level: 0,
		groups: 0,
		scenes: Array(16).fill(255),
		device_type: e === "relay" ? 7 : e === "colour" ? 8 : 0,
		...e === "colour" ? { colour: {
			active: "temperature",
			auto_activation: !0,
			temperature: {
				value: 250,
				coolest: 200,
				warmest: 400,
				physical_coolest: 153,
				physical_warmest: 500
			},
			xy: [2e4, 2e4],
			rgbwaf: [
				0,
				0,
				0
			]
		} } : {}
	} : {
		id: t,
		short_address: null,
		instances: [Number(e)],
		values: [{
			instance: 0,
			resolution: e === "1" ? 1 : e === "3" ? 2 : 10,
			bytes: e === "1" || e === "3" ? [0] : [0, 0]
		}],
		feedback: [{ instance: 0 }]
	};
}
function Te() {
	let [e, t] = W(null), [n, r] = W("runtime"), [i, a] = W(!1), [o, s] = W(""), [c, l] = W(""), [u, d] = W(!1), [f, p] = W(null), [m, h] = W({}), [g, _] = W(null), [y, b] = W(null), x = fe(null), S = async () => {
		t(await X("snapshot")), a(!0);
	};
	G(() => {
		let e = !0, n, r = async () => {
			try {
				if (!Y) {
					Y = (await X("session")).csrf;
					let [t, n] = await Promise.all([X("scenario"), X("examples")]);
					e && (p((e) => e ?? t), h(n));
				}
				let n = await X("snapshot");
				e && (t(n), a(!0));
			} catch {
				Y = "", e && a(!1);
			} finally {
				e && (n = setTimeout(r, 800));
			}
		};
		return r(), () => {
			e = !1, clearTimeout(n);
		};
	}, []);
	let C = async (e, t = "Updated") => {
		d(!0), s(""), l("");
		try {
			await e(), await S(), l(t);
		} catch (e) {
			s(e instanceof Error ? e.message : String(e));
		} finally {
			d(!1);
		}
	}, w = (e, t) => C(() => X("control", e), t), T = (e) => {
		if (f) {
			let t = Z(f);
			e(t), p(t);
		}
	}, E = (e, t) => b({
		text: e,
		action: t
	}), D = (e, t, n, r, i) => {
		i ? E(`Remove ${t.id} from the live installation? Reset restores the saved starting scenario.`, async () => {
			await w({
				op: "remove_fixture",
				id: t.id
			}, "Fixture removed");
		}) : T((t) => {
			t.buses.find((t) => t.number === e)[n ? "inputs" : "outputs"].splice(r, 1);
		});
	}, O = async (e) => {
		g && (g.live ? (await X("control", {
			op: g.input ? "add_input" : "add_output",
			bus: g.bus,
			[g.input ? "input" : "output"]: e
		}), await S(), l("Fixture added live. Capture current state to keep it in the starting scenario.")) : T((t) => {
			var n;
			let r = t.buses.find((e) => e.number === g.bus), i = r[n = g.input ? "inputs" : "outputs"] ?? (r[n] = []);
			g.index === void 0 ? i.push(e) : i[g.index] = e;
		}), _(null));
	}, k = e?.buses ?? [], A = k.reduce((e, t) => e + t.outputs.length + t.inputs.length, 0);
	return /* @__PURE__ */ J(v, { children: [
		/* @__PURE__ */ J("header", { children: [/* @__PURE__ */ J("div", {
			class: "brand",
			children: [/* @__PURE__ */ J("span", {
				class: "logo",
				children: "D"
			}), /* @__PURE__ */ J("div", { children: [/* @__PURE__ */ J("h1", { children: "DALI Emulator" }), /* @__PURE__ */ J("p", { children: "Independent native installation" })] })]
		}), /* @__PURE__ */ J("span", {
			class: `status ${i ? "connected" : ""}`,
			children: i ? "● Emulator online" : "○ Emulator disconnected"
		})] }),
		/* @__PURE__ */ J("main", { children: [
			/* @__PURE__ */ J("div", {
				class: "intro",
				children: [/* @__PURE__ */ J("div", { children: [
					/* @__PURE__ */ J("p", {
						class: "eyebrow",
						children: "YOUR VIRTUAL LIGHTING LAB"
					}),
					/* @__PURE__ */ J("h2", { children: "Build it. Connect it. Observe it." }),
					/* @__PURE__ */ J("p", { children: "Open the bridge in another tab. Its raw TCP HAT connection controls the devices shown here." })
				] }), /* @__PURE__ */ J("div", {
					class: "metrics",
					children: [
						/* @__PURE__ */ J("strong", { children: [A, /* @__PURE__ */ J("small", { children: "fixtures" })] }),
						/* @__PURE__ */ J("strong", { children: [k.length, /* @__PURE__ */ J("small", { children: "buses" })] }),
						/* @__PURE__ */ J("strong", { children: [e?.transport?.connected ? "Linked" : "Waiting", /* @__PURE__ */ J("small", { children: "HAT client" })] })
					]
				})]
			}),
			/* @__PURE__ */ J("nav", {
				"aria-label": "Emulator sections",
				children: [
					["runtime", "Live installation"],
					["scenario", "Starting scenario"],
					["advanced", "Advanced"]
				].map(([e, t]) => /* @__PURE__ */ J("button", {
					class: n === e ? "selected" : "",
					onClick: () => r(e),
					children: t
				}))
			}),
			o && /* @__PURE__ */ J("div", {
				class: "message error",
				role: "alert",
				children: [o, /* @__PURE__ */ J("button", {
					"aria-label": "Dismiss error",
					onClick: () => s(""),
					children: "×"
				})]
			}),
			c && /* @__PURE__ */ J("div", {
				class: "message",
				role: "status",
				children: c
			}),
			!i && /* @__PURE__ */ J("p", {
				class: "message error",
				children: "Controls are unavailable until the emulator reconnects. Your draft stays in this tab."
			}),
			/* @__PURE__ */ J("fieldset", {
				disabled: u || !i,
				class: "workspace",
				children: [
					n === "runtime" && /* @__PURE__ */ J(v, { children: [
						/* @__PURE__ */ J("div", {
							class: "section-title",
							children: [/* @__PURE__ */ J("div", { children: [/* @__PURE__ */ J("h3", { children: "Native device state" }), /* @__PURE__ */ J("p", { children: "Observed directly in the emulator; addresses and names in the bridge stay independent." })] }), /* @__PURE__ */ J("button", {
								onClick: () => void w({ op: "capture_scenario" }, "Current state saved as the starting scenario. Download saved start to export; your draft is unchanged."),
								children: "Save current state as scenario"
							})]
						}),
						k.map((e) => /* @__PURE__ */ J("section", {
							class: "bus",
							children: [
								/* @__PURE__ */ J("div", {
									class: "bus-title",
									children: [/* @__PURE__ */ J("h3", { children: [
										"Bus ",
										e.number + 1,
										" ",
										/* @__PURE__ */ J("span", {
											class: "tag",
											children: e.powered ? "Powered" : "Power off"
										})
									] }), /* @__PURE__ */ J(Oe, { onAdd: (t, n) => _({
										bus: e.number,
										item: t,
										input: n,
										live: !0
									}) })]
								}),
								/* @__PURE__ */ J("div", {
									class: "grid",
									children: [e.outputs.map((t, n) => /* @__PURE__ */ J("article", {
										class: "card",
										children: [
											/* @__PURE__ */ J("div", {
												class: "card-top",
												children: [/* @__PURE__ */ J("span", {
													class: "device-symbol",
													children: t.device_type === 7 ? "⇄" : "◉"
												}), /* @__PURE__ */ J("span", {
													class: "tag",
													children: t.device_type === 7 ? "DT7 relay" : t.device_type === 8 ? "DT8 colour" : "Dimmer"
												})]
											}),
											/* @__PURE__ */ J("h4", { children: t.id }),
											/* @__PURE__ */ J("p", { children: [
												"Address ",
												t.short_address === null ? "unassigned" : t.short_address,
												" · native ID"
											] }),
											/* @__PURE__ */ J("div", {
												class: "level",
												children: [/* @__PURE__ */ J("strong", { children: [t.level, /* @__PURE__ */ J("small", { children: " / 254" })] }), /* @__PURE__ */ J("progress", {
													max: 254,
													value: t.level
												})]
											}),
											t.switching_state && /* @__PURE__ */ J("p", {
												class: "native",
												children: [
													"Contact ",
													/* @__PURE__ */ J("b", { children: t.level ? "CLOSED" : "OPEN" }),
													" · virtual level ",
													t.switching_state.virtual_level
												]
											}),
											t.colour_state && /* @__PURE__ */ J("p", {
												class: "native",
												children: De(t.colour_state.actual)
											}),
											/* @__PURE__ */ J("p", {
												class: "muted",
												children: [
													"Groups: ",
													Ee(t.groups),
													" · Scenes: ",
													t.scenes.filter((e) => e !== 255).length
												]
											}),
											/* @__PURE__ */ J("details", { children: [/* @__PURE__ */ J("summary", { children: "Native settings & scenes" }), /* @__PURE__ */ J("pre", { children: JSON.stringify(t, null, 2) })] }),
											/* @__PURE__ */ J("div", {
												class: "actions",
												children: [/* @__PURE__ */ J("button", {
													onClick: () => void w({
														op: "set_address",
														id: t.id,
														short_address: null
													}, "Address cleared; native identity retained"),
													children: "Lose address"
												}), /* @__PURE__ */ J("button", {
													class: "danger",
													onClick: () => D(e.number, t, !1, n, !0),
													children: "Remove"
												})]
											})
										]
									})), e.inputs.map((t, n) => /* @__PURE__ */ J("article", {
										class: "card input",
										children: [
											/* @__PURE__ */ J("div", {
												class: "card-top",
												children: [/* @__PURE__ */ J("span", {
													class: "device-symbol",
													children: "↳"
												}), /* @__PURE__ */ J("span", {
													class: "tag",
													children: "Input module"
												})]
											}),
											/* @__PURE__ */ J("h4", { children: t.id }),
											/* @__PURE__ */ J("p", { children: [
												"Address ",
												t.short_address === null ? "unassigned" : t.short_address,
												" · ",
												t.instances.length,
												" instances"
											] }),
											t.instances.map((e, n) => /* @__PURE__ */ J(ke, {
												item: t,
												kind: e,
												index: n,
												send: (e) => w({
													op: "stimulate",
													id: t.id,
													instance: n,
													stimulus: e
												}, "Physical stimulus applied")
											})),
											(t.feedback_state ?? []).map((e) => /* @__PURE__ */ J("p", {
												class: "native",
												children: [
													"Feedback ",
													e.instance === null ? "device" : `instance ${e.instance}`,
													": ",
													/* @__PURE__ */ J("b", { children: e.active ? "ON" : "off" })
												]
											})),
											/* @__PURE__ */ J("details", { children: [/* @__PURE__ */ J("summary", { children: "Native settings & values" }), /* @__PURE__ */ J("pre", { children: JSON.stringify(t, null, 2) })] }),
											/* @__PURE__ */ J("div", {
												class: "actions",
												children: [/* @__PURE__ */ J("button", {
													onClick: () => void w({
														op: "set_address",
														id: t.id,
														short_address: null
													}),
													children: "Lose address"
												}), /* @__PURE__ */ J("button", {
													class: "danger",
													onClick: () => D(e.number, t, !0, n, !0),
													children: "Remove"
												})]
											})
										]
									}))]
								}),
								!e.outputs.length && !e.inputs.length && /* @__PURE__ */ J("p", {
									class: "empty",
									children: "This bus is empty. Add a fixture to start testing discovery."
								})
							]
						})),
						/* @__PURE__ */ J("button", {
							onClick: () => {
								let e = [
									0,
									1,
									2,
									3
								].find((e) => !k.some((t) => t.number === e));
								e !== void 0 && w({
									op: "add_bus",
									bus: {
										number: e,
										outputs: [],
										inputs: []
									}
								}, "Bus added live");
							},
							disabled: k.length >= 4,
							children: "+ Add live bus"
						})
					] }),
					n === "scenario" && f && /* @__PURE__ */ J(v, { children: [
						/* @__PURE__ */ J("div", {
							class: "section-title",
							children: /* @__PURE__ */ J("div", { children: [/* @__PURE__ */ J("h3", { children: "Saved start & local draft" }), /* @__PURE__ */ J("p", { children: "Edits below stay local until applied. Applying saves this start and resets every live device; the TCP client reconnects." })] })
						}),
						/* @__PURE__ */ J("div", {
							class: "toolbar",
							children: [
								/* @__PURE__ */ J("button", {
									onClick: () => E("Apply this draft as the saved starting scenario and reset the live installation? The current TCP client will disconnect.", async () => {
										await w({
											op: "apply_scenario",
											scenario: f
										}, "Starting scenario applied; live installation reset");
									}),
									children: "Apply draft & reset"
								}),
								/* @__PURE__ */ J("button", {
									onClick: () => Ce(f, "dali-emulator-scenario.json"),
									children: "Export draft"
								}),
								/* @__PURE__ */ J("button", {
									onClick: () => void C(async () => Ce(await X("scenario"), "dali-emulator-saved-start.json"), "Saved starting scenario downloaded"),
									children: "Download saved start"
								}),
								/* @__PURE__ */ J("button", {
									onClick: () => x.current?.click(),
									children: "Import JSON"
								}),
								/* @__PURE__ */ J("button", {
									onClick: () => void C(async () => p(await X("scenario")), "Draft restored from saved starting scenario"),
									children: "Reload saved start"
								})
							]
						}),
						/* @__PURE__ */ J("input", {
							ref: x,
							class: "hidden",
							type: "file",
							accept: "application/json,.json",
							onChange: async (e) => {
								try {
									let t = e.currentTarget.files?.[0];
									if (!t) return;
									if (t.size > 4194304) throw Error("Scenario exceeds 4 MiB");
									let n = JSON.parse(await t.text());
									Se(n), p(n), l("Imported into local draft. Apply validates the complete scenario before changing live devices.");
								} catch (e) {
									s(String(e));
								}
								x.current && (x.current.value = "");
							}
						}),
						/* @__PURE__ */ J("div", {
							class: "form-row",
							children: [
								/* @__PURE__ */ J("label", { children: ["Start from example", /* @__PURE__ */ J("select", {
									value: "",
									onChange: (e) => p(Z(m[e.currentTarget.value])),
									children: [/* @__PURE__ */ J("option", {
										value: "",
										disabled: !0,
										children: "Choose an example…"
									}), Object.keys(m).map((e) => /* @__PURE__ */ J("option", { children: e }))]
								})] }),
								/* @__PURE__ */ J("label", { children: ["Clock", /* @__PURE__ */ J("select", {
									"aria-label": "Clock",
									value: f.clock ?? "realtime",
									onChange: (e) => T((t) => t.clock = e.currentTarget.value),
									children: [/* @__PURE__ */ J("option", {
										value: "realtime",
										children: "Realtime"
									}), /* @__PURE__ */ J("option", {
										value: "frozen",
										children: "Frozen (advance manually)"
									})]
								})] }),
								/* @__PURE__ */ J("label", { children: ["Repeat terminals", /* @__PURE__ */ J("select", {
									"aria-label": "Repeat terminals",
									value: f.repeat ?? "one_terminal",
									onChange: (e) => T((t) => t.repeat = e.currentTarget.value),
									children: [/* @__PURE__ */ J("option", {
										value: "one_terminal",
										children: "One terminal"
									}), /* @__PURE__ */ J("option", {
										value: "two_terminals",
										children: "Two terminals"
									})]
								})] })
							]
						}),
						f.buses.map((e) => /* @__PURE__ */ J("section", {
							class: "bus",
							children: [/* @__PURE__ */ J("div", {
								class: "bus-title",
								children: [/* @__PURE__ */ J("h3", { children: ["Bus ", e.number + 1] }), /* @__PURE__ */ J("div", {
									class: "actions",
									children: [/* @__PURE__ */ J(Oe, { onAdd: (t, n) => _({
										bus: e.number,
										item: t,
										input: n,
										live: !1
									}) }), /* @__PURE__ */ J("button", {
										class: "danger",
										disabled: f.buses.length <= 1,
										onClick: () => T((t) => t.buses = t.buses.filter((t) => t.number !== e.number)),
										children: "Remove bus"
									})]
								})]
							}), ["outputs", "inputs"].map((t) => (e[t] ?? []).map((n, r) => /* @__PURE__ */ J("div", {
								class: "draft-row",
								children: [/* @__PURE__ */ J("div", { children: [/* @__PURE__ */ J("b", { children: n.id }), /* @__PURE__ */ J("p", { children: [
									t === "inputs" ? "Input module" : `Output type ${n.device_type ?? 0}`,
									" · address ",
									n.short_address ?? "unassigned"
								] })] }), /* @__PURE__ */ J("div", {
									class: "actions",
									children: [/* @__PURE__ */ J("button", {
										onClick: () => _({
											bus: e.number,
											item: Z(n),
											input: t === "inputs",
											index: r,
											live: !1
										}),
										children: "Edit"
									}), /* @__PURE__ */ J("button", {
										class: "danger",
										onClick: () => D(e.number, n, t === "inputs", r, !1),
										children: "Remove"
									})]
								})]
							})))]
						})),
						/* @__PURE__ */ J("button", {
							disabled: f.buses.length >= 4,
							onClick: () => T((e) => {
								let t = [
									0,
									1,
									2,
									3
								].find((t) => !e.buses.some((e) => e.number === t));
								t !== void 0 && e.buses.push({
									number: t,
									outputs: [],
									inputs: []
								});
							}),
							children: "+ Add bus to draft"
						}),
						/* @__PURE__ */ J("details", {
							class: "raw",
							children: [/* @__PURE__ */ J("summary", { children: "Complete scenario JSON (advanced)" }), /* @__PURE__ */ J($, {
								value: f,
								apply: (e) => {
									Se(e), p(e);
								},
								label: "Scenario JSON"
							})]
						})
					] }),
					n === "advanced" && /* @__PURE__ */ J(v, { children: [
						/* @__PURE__ */ J("h3", { children: "Transport & model controls" }),
						/* @__PURE__ */ J("p", { children: "HAT reboot preserves fixtures and native settings. Scenario reset restores the saved start and disconnects TCP." }),
						/* @__PURE__ */ J("div", {
							class: "toolbar",
							children: [
								/* @__PURE__ */ J("button", {
									onClick: () => void w({ op: "reboot" }, "HAT rebooted; physical state retained"),
									children: "Reboot HAT"
								}),
								/* @__PURE__ */ J("button", {
									class: "danger",
									onClick: () => E("Reset all live devices to the saved starting scenario? The TCP connection will close.", async () => {
										await w({ op: "reset_scenario" }, "Saved starting scenario restored");
									}),
									children: "Reset scenario"
								}),
								/* @__PURE__ */ J("button", {
									onClick: () => void w({
										op: "advance_time",
										millis: 1e3
									}, "Model advanced one second"),
									children: "Advance 1 second"
								}),
								/* @__PURE__ */ J("button", {
									onClick: () => void w({
										op: "advance_time",
										millis: 6e4
									}, "Model advanced one minute"),
									children: "Advance 1 minute"
								})
							]
						}),
						/* @__PURE__ */ J("p", {
							class: "native",
							children: [
								"Model time: ",
								e?.time_ms ?? 0,
								" ms · Unsupported commands: ",
								e?.unsupported_commands ?? 0
							]
						}),
						/* @__PURE__ */ J("h3", { children: "Bus power" }),
						k.map((e) => /* @__PURE__ */ J("button", {
							onClick: () => void w({
								op: "bus_power",
								bus: e.number,
								powered: !e.powered
							}),
							children: [
								"Bus ",
								e.number + 1,
								": turn ",
								e.powered ? "off" : "on"
							]
						})),
						/* @__PURE__ */ J("h3", { children: "Next terminal fault" }),
						/* @__PURE__ */ J("p", { children: "Affects the next J/N/X/Z response. Echoes and physical input events remain separate." }),
						/* @__PURE__ */ J("div", {
							class: "toolbar",
							children: [
								[
									"drop",
									"receive_collision",
									"transmit_collision"
								].map((e) => /* @__PURE__ */ J("button", {
									onClick: () => void w({
										op: "next_fault",
										effect: { kind: e }
									}, "Next-response fault armed"),
									children: e.replaceAll("_", " ")
								})),
								/* @__PURE__ */ J("button", {
									onClick: () => void w({
										op: "next_fault",
										effect: {
											kind: "delay",
											millis: 1e3
										}
									}),
									children: "Delay 1 second"
								}),
								/* @__PURE__ */ J("button", {
									onClick: () => void w({
										op: "next_fault",
										effect: {
											kind: "truncate",
											length: 1
										}
									}),
									children: "Truncate to 1 byte"
								})
							]
						}),
						/* @__PURE__ */ J("details", { children: [
							/* @__PURE__ */ J("summary", { children: "Raw compatibility controls" }),
							/* @__PURE__ */ J("p", { children: "Raw input_event bypasses enabled/filter/scheme settings. set_input_value emits no event. These controls are for protocol testing." }),
							/* @__PURE__ */ J($, {
								value: {
									op: "input_event",
									bus: 0,
									frame: 32769
								},
								apply: (e) => void w(e),
								label: "Raw control JSON"
							})
						] }),
						/* @__PURE__ */ J("p", {
							class: "muted",
							children: "Emulator evidence is separate from physical HAT certification. Fades are immediate; sensor values use native raw units. Large time jumps coalesce repeated reports. No MQTT or real UART is opened."
						})
					] })
				]
			})
		] }),
		/* @__PURE__ */ J("footer", { children: "ATX I4 · independent wire & physical model · local testing" }),
		g && /* @__PURE__ */ J(Ae, {
			edit: g,
			close: () => _(null),
			save: O
		}, g.item.id + String(g.index)),
		y && /* @__PURE__ */ J("div", {
			class: "overlay",
			children: /* @__PURE__ */ J("div", {
				role: "dialog",
				"aria-modal": "true",
				"aria-label": "Confirm installation change",
				class: "dialog",
				children: [
					/* @__PURE__ */ J("h3", { children: "Confirm installation change" }),
					/* @__PURE__ */ J("p", { children: y.text }),
					/* @__PURE__ */ J("div", {
						class: "actions",
						children: [/* @__PURE__ */ J("button", {
							onClick: () => b(null),
							children: "Cancel"
						}), /* @__PURE__ */ J("button", {
							class: "primary",
							onClick: () => {
								let e = y.action;
								b(null), e();
							},
							children: "Confirm"
						})]
					})
				]
			})
		})
	] });
}
function Ee(e) {
	return Array.from({ length: 16 }, (e, t) => t).filter((t) => e & 1 << t).join(", ") || "none";
}
function De(e) {
	return e.active === "temperature" ? `Temperature: ${e.temperature.value} mirek` : e.active === "xy" ? `xy: ${e.xy.join(", ")}` : `RGBWAF: ${e.rgbwaf.join(", ")}`;
}
function Oe({ onAdd: e }) {
	return /* @__PURE__ */ J("label", {
		class: "add",
		children: ["+ Add fixture", /* @__PURE__ */ J("select", {
			"aria-label": "Add fixture type",
			value: "",
			onChange: (t) => {
				let n = t.currentTarget.value;
				e(we(n), ![
					"dimmer",
					"relay",
					"colour"
				].includes(n));
			},
			children: [
				/* @__PURE__ */ J("option", {
					value: "",
					disabled: !0,
					children: "Choose type…"
				}),
				/* @__PURE__ */ J("option", {
					value: "dimmer",
					children: "Dimmer"
				}),
				/* @__PURE__ */ J("option", {
					value: "relay",
					children: "DT7 relay"
				}),
				/* @__PURE__ */ J("option", {
					value: "colour",
					children: "DT8 colour"
				}),
				[
					1,
					2,
					3,
					4
				].map((e) => /* @__PURE__ */ J("option", {
					value: e,
					children: Q[e]
				}))
			]
		})]
	});
}
function ke({ item: e, kind: t, index: n, send: r }) {
	let [i, a] = W("0"), o = e.physical?.[n], s = e.settings?.[n];
	return /* @__PURE__ */ J("div", {
		class: "instance",
		children: [/* @__PURE__ */ J("div", { children: [/* @__PURE__ */ J("b", { children: [
			n,
			": ",
			Q[t] ?? `Type ${t}`
		] }), /* @__PURE__ */ J("span", {
			class: "tag",
			children: s?.enabled ? "enabled" : "disabled"
		})] }), t === 1 ? /* @__PURE__ */ J(v, { children: [/* @__PURE__ */ J("p", { children: o?.pressed ? "Pressed" : "Released" }), /* @__PURE__ */ J("div", {
			class: "actions",
			children: [/* @__PURE__ */ J("button", {
				disabled: o?.pressed,
				onClick: () => void r({
					kind: "button",
					pressed: !0
				}),
				children: "Press"
			}), /* @__PURE__ */ J("button", {
				disabled: !o?.pressed,
				onClick: () => void r({
					kind: "button",
					pressed: !1
				}),
				children: "Release"
			})]
		})] }) : t === 3 ? /* @__PURE__ */ J(v, { children: [/* @__PURE__ */ J("p", { children: [
			o?.occupied ? "Occupied" : "Vacant",
			" · ",
			o?.movement ? "movement" : "no movement"
		] }), /* @__PURE__ */ J("div", {
			class: "actions",
			children: [/* @__PURE__ */ J("button", {
				onClick: () => void r({
					kind: "occupancy",
					occupied: !0,
					movement: !0
				}),
				children: "Movement"
			}), /* @__PURE__ */ J("button", {
				onClick: () => void r({
					kind: "occupancy",
					occupied: !1,
					movement: !1
				}),
				children: "No presence"
			})]
		})] }) : t === 2 || t === 4 ? /* @__PURE__ */ J(v, { children: [
			/* @__PURE__ */ J("label", { children: ["Native raw value (0–1023)", /* @__PURE__ */ J("input", {
				type: "number",
				min: "0",
				max: "1023",
				value: i,
				onInput: (e) => a(e.currentTarget.value)
			})] }),
			/* @__PURE__ */ J("button", {
				onClick: () => void r({
					kind: "value",
					raw_10: Number(i)
				}),
				children: "Set physical value"
			}),
			/* @__PURE__ */ J("small", { children: ["Current raw10: ", o?.raw_10 ?? 0] })
		] }) : /* @__PURE__ */ J("p", { children: "Unknown native type; use advanced raw controls." })]
	});
}
function $({ value: e, apply: t, label: n }) {
	let r = JSON.stringify(e, null, 2), [i, a] = W(r), [o, s] = W(r), [c, l] = W(""), [u, d] = W(!1);
	return G(() => {
		r !== o && (i === o ? (a(r), s(r), d(!1)) : d(!0));
	}, [r]), /* @__PURE__ */ J(v, { children: [
		/* @__PURE__ */ J("label", { children: [n, /* @__PURE__ */ J("textarea", {
			rows: 12,
			value: i,
			onInput: (e) => a(e.currentTarget.value)
		})] }),
		u && /* @__PURE__ */ J("p", {
			role: "alert",
			children: "This record changed outside the JSON editor. Copy your JSON edits if needed, then reload the current record."
		}),
		c && /* @__PURE__ */ J("p", {
			role: "alert",
			children: c
		}),
		/* @__PURE__ */ J("button", {
			type: "button",
			disabled: u,
			onClick: () => {
				try {
					let e = JSON.parse(i);
					t(e);
					let n = JSON.stringify(e, null, 2);
					a(n), s(n), l("");
				} catch (e) {
					l(String(e));
				}
			},
			children: "Use JSON"
		}),
		/* @__PURE__ */ J("button", {
			type: "button",
			onClick: () => {
				a(r), s(r), d(!1), l("");
			},
			children: "Reload current JSON"
		})
	] });
}
function Ae({ edit: e, close: t, save: n }) {
	let [r, i] = W(Z(e.item)), [a, o] = W(""), [s, c] = W(!1), l = (e, t) => i({
		...r,
		[e]: t
	});
	return G(() => {
		let e = (e) => {
			e.key === "Escape" && !s && t();
		};
		return document.addEventListener("keydown", e), () => document.removeEventListener("keydown", e);
	}, [s]), /* @__PURE__ */ J("div", {
		class: "overlay",
		children: /* @__PURE__ */ J("form", {
			role: "dialog",
			"aria-modal": "true",
			"aria-label": "Fixture editor",
			class: "dialog",
			onSubmit: async (e) => {
				e.preventDefault(), c(!0);
				try {
					await n(r);
				} catch (e) {
					o(String(e));
				} finally {
					c(!1);
				}
			},
			children: [
				/* @__PURE__ */ J("div", {
					class: "section-title",
					children: [/* @__PURE__ */ J("h3", { children: e.live ? "Add live fixture" : "Edit draft fixture" }), /* @__PURE__ */ J("button", {
						type: "button",
						"aria-label": "Close fixture editor",
						disabled: s,
						onClick: t,
						children: "×"
					})]
				}),
				/* @__PURE__ */ J("p", { children: e.live ? "This adds only this device; existing fixtures keep their state." : "Changes stay in the draft until Apply draft & reset." }),
				/* @__PURE__ */ J("label", { children: ["Stable emulator ID", /* @__PURE__ */ J("input", {
					required: !0,
					maxLength: 128,
					value: r.id,
					onInput: (e) => l("id", e.currentTarget.value)
				})] }),
				/* @__PURE__ */ J("label", { children: ["Initial short address (blank = unaddressed)", /* @__PURE__ */ J("input", {
					type: "number",
					min: "0",
					max: "63",
					value: r.short_address ?? "",
					onInput: (e) => l("short_address", e.currentTarget.value === "" ? null : Number(e.currentTarget.value))
				})] }),
				e.input ? /* @__PURE__ */ J(v, { children: [
					/* @__PURE__ */ J("h4", { children: "Input instances" }),
					r.instances.map((e, t) => /* @__PURE__ */ J("div", {
						class: "instance-row",
						children: [/* @__PURE__ */ J("label", { children: [
							"Instance ",
							t,
							/* @__PURE__ */ J("select", {
								"aria-label": `Instance ${t}`,
								value: e,
								onChange: (e) => {
									let n = Z(r);
									n.instances[t] = Number(e.currentTarget.value), delete n.initial_settings, delete n.initial_physical, i(n);
								},
								children: [[
									1,
									2,
									3,
									4
								].map((e) => /* @__PURE__ */ J("option", {
									value: e,
									children: Q[e]
								})), ![
									1,
									2,
									3,
									4
								].includes(e) && /* @__PURE__ */ J("option", {
									value: e,
									children: ["Unknown native type ", e]
								})]
							})
						] }), /* @__PURE__ */ J("button", {
							type: "button",
							onClick: () => {
								let e = Z(r);
								e.instances.splice(t, 1), e.values = (e.values ?? []).filter((e) => e.instance !== t).map((e) => ({
									...e,
									instance: e.instance > t ? e.instance - 1 : e.instance
								})), e.feedback = (e.feedback ?? []).filter((e) => e.instance !== t).map((e) => ({
									...e,
									instance: e.instance !== null && e.instance > t ? e.instance - 1 : e.instance
								})), delete e.initial_settings, delete e.initial_physical, i(e);
							},
							children: "Remove instance"
						})]
					})),
					/* @__PURE__ */ J("button", {
						type: "button",
						disabled: r.instances.length >= 32,
						onClick: () => {
							let e = Z(r);
							e.instances.push(1), delete e.initial_settings, delete e.initial_physical, i(e);
						},
						children: "+ Add button instance"
					}),
					/* @__PURE__ */ J("p", {
						class: "muted",
						children: "Value/resolution, feedback profiles and optional native settings are editable in the full record below."
					})
				] }) : /* @__PURE__ */ J(v, { children: [
					/* @__PURE__ */ J("div", {
						class: "form-row",
						children: [
							/* @__PURE__ */ J("label", { children: ["Native device type", /* @__PURE__ */ J("input", {
								type: "number",
								min: "0",
								max: "255",
								value: r.device_type ?? 0,
								onInput: (e) => l("device_type", Number(e.currentTarget.value))
							})] }),
							/* @__PURE__ */ J("label", { children: ["Initial actual level", /* @__PURE__ */ J("input", {
								type: "number",
								min: "0",
								max: "254",
								value: r.level ?? 0,
								onInput: (e) => l("level", Number(e.currentTarget.value))
							})] }),
							/* @__PURE__ */ J("label", { children: ["Group mask (0–65535)", /* @__PURE__ */ J("input", {
								type: "number",
								min: "0",
								max: "65535",
								value: r.groups ?? 0,
								onInput: (e) => l("groups", Number(e.currentTarget.value))
							})] })
						]
					}),
					/* @__PURE__ */ J("label", { children: ["16 scene levels (255 = no participation)", /* @__PURE__ */ J("input", {
						value: (r.scenes ?? Array(16).fill(255)).join(","),
						onInput: (e) => l("scenes", e.currentTarget.value.split(",").map(Number))
					})] }),
					r.colour && /* @__PURE__ */ J("details", {
						open: !0,
						children: [/* @__PURE__ */ J("summary", { children: "Colour capabilities & initial values" }), /* @__PURE__ */ J($, {
							value: r.colour,
							apply: (e) => l("colour", e),
							label: "Colour profile JSON"
						})]
					})
				] }),
				/* @__PURE__ */ J("details", { children: [/* @__PURE__ */ J("summary", { children: "Full fixture record (advanced)" }), /* @__PURE__ */ J($, {
					value: r,
					apply: (t) => {
						xe(t, e.input), i(t);
					},
					label: "Fixture JSON"
				})] }),
				a && /* @__PURE__ */ J("p", {
					class: "error message",
					role: "alert",
					children: a
				}),
				/* @__PURE__ */ J("div", {
					class: "dialog-footer",
					children: [/* @__PURE__ */ J("button", {
						type: "button",
						disabled: s,
						onClick: t,
						children: "Cancel"
					}), /* @__PURE__ */ J("button", {
						class: "primary",
						disabled: s,
						type: "submit",
						children: s ? "Saving…" : e.live ? "Add live fixture" : "Save to draft"
					})]
				})
			]
		})
	});
}
P(/* @__PURE__ */ J(Te, {}), document.getElementById("app"));
