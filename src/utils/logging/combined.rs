//! A layer that forwards to a vec of sub-layers to allow layers to be dynamically defined.
//!
//! This module provides a `Layer` type which wraps a vec of Box<dyn Layer>,
//! allowing the wrapped layers to be dynamically created at runtime.
//!
use std::ops::ControlFlow;

use tracing_core::{
    callsite, span,
    subscriber::{Interest, Subscriber},
    Event, Metadata,
};
use tracing_subscriber::layer::{Context, Layer as LayerTrait};

type BoxedLayer<S> = Box<dyn LayerTrait<S> + Send + Sync + 'static>;

/// Wraps a vec of `Layer`s
pub struct Layer<S> {
    inners: Vec<BoxedLayer<S>>,
}

// ===== impl Layer =====

impl<S> LayerTrait<S> for Layer<S>
where
    S: Subscriber,
{
    #[inline]
    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        let res = self.inners.iter().try_fold(Interest::always(), |acc, layer| {
            if acc.is_never() || acc.is_sometimes() {
                // short circuit logic if previous layer has disabled the callsite,
                // or it returns "sometimes", in which case we return that to ensure
                // filters are reevaluated.
                ControlFlow::Break(acc)
            } else {
                // let the next layer to weight in
                ControlFlow::Continue(layer.register_callsite(metadata))
            }
        });
        match res {
            ControlFlow::Break(res) => res,
            ControlFlow::Continue(res) => res,
        }
    }

    #[inline]
    fn enabled(&self, metadata: &Metadata<'_>, ctx: Context<'_, S>) -> bool {
        self.inners.iter().all(|layer| layer.enabled(metadata, ctx.clone()))
    }

    #[inline]
    fn new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        for layer in self.inners.iter() {
            layer.new_span(attrs, id, ctx.clone());
        }
    }

    #[inline]
    fn on_record(&self, span: &span::Id, values: &span::Record<'_>, ctx: Context<'_, S>) {
        for layer in self.inners.iter() {
            layer.on_record(span, values, ctx.clone());
        }
    }

    #[inline]
    fn on_follows_from(&self, span: &span::Id, follows: &span::Id, ctx: Context<'_, S>) {
        for layer in self.inners.iter() {
            layer.on_follows_from(span, follows, ctx.clone());
        }
    }

    #[inline]
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        for layer in self.inners.iter() {
            layer.on_event(event, ctx.clone());
        }
    }

    #[inline]
    fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
        for layer in self.inners.iter() {
            layer.on_enter(id, ctx.clone());
        }
    }

    #[inline]
    fn on_exit(&self, id: &span::Id, ctx: Context<'_, S>) {
        for layer in self.inners.iter() {
            layer.on_exit(id, ctx.clone());
        }
    }

    #[inline]
    fn on_close(&self, id: span::Id, ctx: Context<'_, S>) {
        for layer in self.inners.iter() {
            layer.on_close(id.clone(), ctx.clone());
        }
    }

    #[inline]
    fn on_id_change(&self, old: &span::Id, new: &span::Id, ctx: Context<'_, S>) {
        for layer in self.inners.iter() {
            layer.on_id_change(old, new, ctx.clone());
        }
    }
}

impl<S> Layer<S>
where
    S: Subscriber,
{
    /// An empty layer
    pub fn empty() -> Self {
        Self { inners: vec![] }
    }

    pub fn new<T, L>(iter: T) -> Self
    where
        T: IntoIterator<Item = L>,
        L: LayerTrait<S> + Send + Sync + 'static,
    {
        let mut this = Self::empty();
        this.extend(iter);
        this
    }

    pub fn add<L>(&mut self, layer: L) -> &mut Self
    where
        L: LayerTrait<S> + Send + Sync + 'static,
    {
        self.inners.push(Box::new(layer));
        callsite::rebuild_interest_cache();
        self
    }
}

impl<S, L> Extend<L> for Layer<S>
where
    S: Subscriber,
    L: LayerTrait<S> + Send + Sync + 'static,
{
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = L>,
    {
        let iter = iter.into_iter().map(|l| -> BoxedLayer<S> { Box::new(l) });
        self.inners.extend(iter);
        callsite::rebuild_interest_cache();
    }
}
